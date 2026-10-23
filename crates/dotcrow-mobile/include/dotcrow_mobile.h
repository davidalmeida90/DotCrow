/*
 * dotcrow_mobile.h — ABI C do núcleo do túnel do DotCrow.
 *
 * O app iOS (SwiftUI) importa este cabeçalho (via module.modulemap) e linka a
 * libdotcrow_mobile.a. Toda a criptografia do túnel (handshake de ponta a ponta e cifra
 * da sessão) vive do lado Rust; o app só cuida do WebSocket com o relay e da moldura
 * `tag || payload` dos frames.
 *
 * Convenção: as funções devolvem DC_OK (0) em sucesso ou DC_ERRO (1) em erro, escrevendo
 * a mensagem (UTF-8, sem NUL) em `out_err`. Todo DcBytes devolvido é liberado com
 * dc_bytes_free; toda sessão, com dc_sessao_free.
 */
#ifndef DOTCROW_MOBILE_H
#define DOTCROW_MOBILE_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

#define DC_OK 0
#define DC_ERRO 1

/* Bytes de uma via só, de posse do Rust. Libere com dc_bytes_free. */
typedef struct DcBytes {
    uint8_t *ptr; /* início (ou NULL se vazio) */
    size_t len;   /* bytes válidos */
    size_t cap;   /* capacidade (uso interno ao liberar) */
} DcBytes;

/* Sessão cifrada do túnel (opaca). */
typedef struct DcSessao DcSessao;

/*
 * 1º pareamento (por QR). `sala` e `token` (pt) são strings C; `janela` (w) vem do QR.
 * `hello` é o payload do frame HELLO (sem o byte de tag). Em sucesso, *out_key recebe o
 * frame KEY e *out_sessao a sessão.
 */
int32_t dc_parear(const char *sala, const char *token, uint64_t janela,
                  const uint8_t *hello_ptr, size_t hello_len,
                  DcBytes *out_key, DcSessao **out_sessao, DcBytes *out_err);

/* Reconexão sem QR, com o código durável guardado no Keychain. */
int32_t dc_reconectar(const char *sala, const char *codigo,
                      const uint8_t *hello_ptr, size_t hello_len,
                      DcBytes *out_key, DcSessao **out_sessao, DcBytes *out_err);

/* Cifra um JSON (requisição da API) num frame DATA pronto para o relay. */
int32_t dc_sessao_cifrar(const DcSessao *sessao,
                         const uint8_t *json_ptr, size_t json_len,
                         DcBytes *out, DcBytes *out_err);

/* Abre o payload (sem tag) de um frame DATA/OK/CRED vindo do computador. */
int32_t dc_sessao_decifrar(const DcSessao *sessao,
                           const uint8_t *payload_ptr, size_t payload_len,
                           DcBytes *out, DcBytes *out_err);

/* Lê dotcrow://p?r=&s=&pt=&w= e devolve as partes. */
int32_t dc_parse_pairing(const char *uri,
                         DcBytes *out_relay, DcBytes *out_sala, DcBytes *out_token,
                         uint64_t *out_janela, DcBytes *out_err);

/* Libera um DcBytes (ptr NULL = no-op). Não chame duas vezes. */
void dc_bytes_free(DcBytes b);

/* Libera uma sessão (NULL = no-op). */
void dc_sessao_free(DcSessao *sessao);

/* Versão da lib, string C estática (não liberar). */
const char *dc_versao(void);

#ifdef __cplusplus
}
#endif

#endif /* DOTCROW_MOBILE_H */
