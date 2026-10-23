<?php
/**
 * relay.php — ponto de encontro (relay) do túnel do DotCrow, em PHP puro.
 *
 * É a alternativa ao servidor Rust `dotcrow-relay` para quem prefere só subir um arquivo
 * no domínio (hospedagem PHP comum) em vez de rodar um binário. O papel é o mesmo: repassar
 * bytes entre o computador (host) e o celular (guest) que estão na mesma "sala". Como aqui
 * não dá para manter um WebSocket aberto em PHP comum, a conversa é por HTTP com long-poll:
 * cada lado ENVIA frames (POST) e BUSCA os do outro (GET), esperando um pouco se não houver
 * nada ainda.
 *
 * ZERO-CONHECIMENTO: tudo que passa aqui é opaco. O frame já vem cifrado de ponta a ponta
 * (envelope pós-quântico + sessão XChaCha20-Poly1305); este arquivo só vê base64 embaralhado,
 * o id da sala e para que lado mandar. Não há chave nenhuma aqui, e nada é descriptografável.
 *
 * ── API ───────────────────────────────────────────────────────────────────────────────
 *   POST relay.php?sala=<room>&de=<host|guest>
 *        corpo = UM frame em base64 (texto). Vai para a caixa do OUTRO lado.
 *        resposta: {"ok":true,"seq":N}
 *
 *   GET  relay.php?sala=<room>&para=<host|guest>&desde=<n>&espera=<segundos>
 *        devolve os frames da caixa de <para> com seq > desde (long-poll até `espera` s).
 *        resposta: {"frames":[{"seq":N,"b64":"..."}],"ultimo":M}
 *        O cliente guarda `ultimo` e manda como `desde` na próxima vez (idempotente).
 *
 *   GET  relay.php?saude=1  → {"ok":true,"servico":"dotcrow-relay-php"}
 *
 * As caixas: o que o host ENVIA vai para a caixa "guest" (o celular lê); o que o guest
 * envia vai para a caixa "host" (o computador lê). A sala vem do QR (room_id da identidade
 * do computador), então os dois lados se acham sem cadastro nenhum.
 *
 * Limpeza: cada sala expira depois de ~5 min sem uso (os arquivos são apagados). Frames
 * têm tamanho máximo. Nada fica guardado além do necessário para a entrega.
 */

declare(strict_types=1);

// ── Ajustes ────────────────────────────────────────────────────────────────────────────
const TTL_SALA      = 300;          // segundos sem uso até a sala ser apagada
const ESPERA_MAX    = 25;           // teto do long-poll (abaixo do timeout do PHP/proxy)
const FRAME_MAX     = 2 * 1024 * 1024; // tamanho máximo de um frame (base64)
const CAIXA_MAX     = 32 * 1024 * 1024; // teto da caixa, evita crescer sem limite

header('Content-Type: application/json; charset=utf-8');
header('Cache-Control: no-store');
// App nativo não usa CORS; liberamos mesmo assim para facilitar testes por navegador.
header('Access-Control-Allow-Origin: *');
header('Access-Control-Allow-Methods: GET, POST, OPTIONS');
header('Access-Control-Allow-Headers: Content-Type');
if (($_SERVER['REQUEST_METHOD'] ?? 'GET') === 'OPTIONS') {
    http_response_code(204);
    exit;
}

ignore_user_abort(true);
@set_time_limit(ESPERA_MAX + 10);

// ── Health check ─────────────────────────────────────────────────────────────────────
if (isset($_GET['saude'])) {
    echo json_encode(['ok' => true, 'servico' => 'dotcrow-relay-php']);
    exit;
}

// ── Validação dos parâmetros comuns ─────────────────────────────────────────────────────
$sala = (string)($_GET['sala'] ?? '');
if (!preg_match('/^[A-Za-z0-9_-]{8,128}$/', $sala)) {
    responder(400, ['erro' => 'sala inválida']);
}

$dir = diretorio_da_sala($sala);
varrer_salas_velhas(dirname($dir)); // faxina ocasional

$metodo = $_SERVER['REQUEST_METHOD'] ?? 'GET';

if ($metodo === 'POST') {
    $de = (string)($_GET['de'] ?? '');
    if ($de !== 'host' && $de !== 'guest') {
        responder(400, ['erro' => 'de precisa ser host ou guest']);
    }
    // O que um lado envia vai para a caixa do outro.
    $caixa = ($de === 'host') ? 'guest' : 'host';

    $corpo = file_get_contents('php://input');
    if ($corpo === false || $corpo === '') {
        responder(400, ['erro' => 'corpo vazio']);
    }
    $corpo = trim($corpo);
    if (strlen($corpo) > FRAME_MAX) {
        responder(413, ['erro' => 'frame grande demais']);
    }
    if (base64_decode($corpo, true) === false) {
        responder(400, ['erro' => 'frame precisa ser base64']);
    }

    $seq = enfileirar($dir, $caixa, $corpo);
    if ($seq < 0) {
        responder(507, ['erro' => 'caixa cheia']);
    }
    responder(200, ['ok' => true, 'seq' => $seq]);
}

// ── GET: long-poll ───────────────────────────────────────────────────────────────────
$para = (string)($_GET['para'] ?? '');
if ($para !== 'host' && $para !== 'guest') {
    responder(400, ['erro' => 'para precisa ser host ou guest']);
}
$desde  = max(0, (int)($_GET['desde'] ?? 0));
$espera = min(ESPERA_MAX, max(0, (int)($_GET['espera'] ?? 20)));

$fim = microtime(true) + $espera;
do {
    [$frames, $ultimo] = ler_desde($dir, $para, $desde);
    if (!empty($frames) || $espera === 0) {
        responder(200, ['frames' => $frames, 'ultimo' => $ultimo]);
    }
    if (connection_aborted()) {
        exit;
    }
    usleep(150000); // 150 ms
} while (microtime(true) < $fim);

responder(200, ['frames' => [], 'ultimo' => $desde]);

// ── Funções ────────────────────────────────────────────────────────────────────────────

/** Diretório da sala (nome derivado por hash, para não expor o room_id no sistema de arquivos). */
function diretorio_da_sala(string $sala): string {
    $base = base_dados();
    $nome = substr(hash('sha256', $sala), 0, 32);
    $dir = $base . '/' . $nome;
    if (!is_dir($dir)) {
        @mkdir($dir, 0700, true);
    }
    return $dir;
}

/** Pasta-base dos dados do relay (fora do docroot quando dá; senão, ao lado do script). */
function base_dados(): string {
    $cand = sys_get_temp_dir() . '/dotcrow-relay';
    if (@mkdir($cand, 0700, true) || is_dir($cand)) {
        return $cand;
    }
    $local = __DIR__ . '/.relay-dados';
    @mkdir($local, 0700, true);
    // Barra o acesso web a estes arquivos (mesmo sendo só ruído cifrado).
    @file_put_contents($local . '/.htaccess', "Require all denied\nDeny from all\n");
    return $local;
}

/** Acrescenta um frame (base64) à caixa. Devolve o seq (nº da linha) ou -1 se cheia. */
function enfileirar(string $dir, string $caixa, string $b64): int {
    $arq = $dir . '/' . $caixa . '.log';
    $fp = fopen($arq, 'c+');
    if ($fp === false) {
        return -1;
    }
    try {
        flock($fp, LOCK_EX);
        $tam = fstat($fp)['size'] ?? 0;
        if ($tam > CAIXA_MAX) {
            return -1;
        }
        fseek($fp, 0, SEEK_END);
        fwrite($fp, $b64 . "\n");
        fflush($fp);
        @touch($dir);
        return contar_linhas($arq);
    } finally {
        flock($fp, LOCK_UN);
        fclose($fp);
    }
}

/** Lê os frames da caixa com seq > $desde. Devolve [frames, ultimo_seq]. */
function ler_desde(string $dir, string $caixa, int $desde): array {
    $arq = $dir . '/' . $caixa . '.log';
    if (!is_file($arq)) {
        return [[], $desde];
    }
    $fp = fopen($arq, 'r');
    if ($fp === false) {
        return [[], $desde];
    }
    $frames = [];
    $seq = 0;
    try {
        flock($fp, LOCK_SH);
        while (($linha = fgets($fp)) !== false) {
            $seq++;
            if ($seq <= $desde) {
                continue;
            }
            $linha = trim($linha);
            if ($linha !== '') {
                $frames[] = ['seq' => $seq, 'b64' => $linha];
            }
        }
    } finally {
        flock($fp, LOCK_UN);
        fclose($fp);
    }
    @touch($dir);
    return [$frames, $seq];
}

function contar_linhas(string $arq): int {
    $n = 0;
    $fp = fopen($arq, 'r');
    if ($fp === false) {
        return 0;
    }
    while (fgets($fp) !== false) {
        $n++;
    }
    fclose($fp);
    return $n;
}

/** Apaga salas sem uso há mais de TTL_SALA (chamada eventual, 1 em cada ~20 requisições). */
function varrer_salas_velhas(string $base): void {
    if (random_int(1, 20) !== 1 || !is_dir($base)) {
        return;
    }
    $agora = time();
    foreach (glob($base . '/*', GLOB_ONLYDIR) ?: [] as $d) {
        if ($agora - (int)@filemtime($d) > TTL_SALA) {
            foreach (glob($d . '/*') ?: [] as $f) {
                @unlink($f);
            }
            @unlink($d . '/.htaccess');
            @rmdir($d);
        }
    }
}

function responder(int $codigo, array $corpo): void {
    http_response_code($codigo);
    echo json_encode($corpo);
    exit;
}
