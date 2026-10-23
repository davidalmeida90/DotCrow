import Foundation
import DotCrowMobile

/// Erro vindo do núcleo Rust (mensagem já em português, pronta para a tela).
struct DotCrowError: LocalizedError {
    let mensagem: String
    var errorDescription: String? { mensagem }
}

/// Converte um `DcBytes` devolvido pelo Rust em `Data` e **libera** o buffer.
/// Depois desta chamada o `DcBytes` não deve mais ser usado.
private func tomar(_ b: DcBytes) -> Data {
    guard let p = b.ptr, b.len > 0 else {
        dc_bytes_free(b)
        return Data()
    }
    let d = Data(bytes: p, count: b.len)
    dc_bytes_free(b)
    return d
}

/// Lê a mensagem de erro de um `DcBytes` (UTF-8) e libera.
private func tomarErro(_ b: DcBytes) -> String {
    let d = tomar(b)
    return String(data: d, encoding: .utf8) ?? "erro desconhecido"
}

/// A sessão cifrada do túnel. Dona do ponteiro Rust; libera ao ser destruída.
final class SessaoTunel {
    fileprivate let ptr: OpaquePointer

    fileprivate init(_ p: OpaquePointer) { self.ptr = p }

    deinit { dc_sessao_free(ptr) }

    /// Cifra um JSON (requisição da API do painel) num frame `DATA` pronto para o relay.
    func cifrar(_ json: Data) throws -> Data {
        var saida = DcBytes()
        var erro = DcBytes()
        let rc = json.withUnsafeBytes { bruto -> Int32 in
            dc_sessao_cifrar(ptr,
                             bruto.bindMemory(to: UInt8.self).baseAddress,
                             json.count,
                             &saida, &erro)
        }
        guard rc == DC_OK else { throw DotCrowError(mensagem: tomarErro(erro)) }
        dc_bytes_free(erro)
        return tomar(saida)
    }

    /// Abre o payload (sem o byte de tag) de um frame `DATA`/`OK`/`CRED` do computador.
    func decifrar(_ payload: Data) throws -> Data {
        var saida = DcBytes()
        var erro = DcBytes()
        let rc = payload.withUnsafeBytes { bruto -> Int32 in
            dc_sessao_decifrar(ptr,
                               bruto.bindMemory(to: UInt8.self).baseAddress,
                               payload.count,
                               &saida, &erro)
        }
        guard rc == DC_OK else { throw DotCrowError(mensagem: tomarErro(erro)) }
        dc_bytes_free(erro)
        return tomar(saida)
    }
}

/// O handshake do lado do celular, todo em Rust. Dá o frame `KEY` para mandar ao relay
/// e a sessão cifrada derivada.
enum Handshake {
    /// 1º pareamento (por QR): `token` (pt) e `janela` (w) vieram do QR.
    static func parear(sala: String, token: String, janela: UInt64, hello: Data)
        throws -> (key: Data, sessao: SessaoTunel)
    {
        try executar(hello: hello) { out_key, out_sessao, out_err in
            sala.withCString { cSala in
                token.withCString { cToken in
                    hello.withUnsafeBytes { h in
                        dc_parear(cSala, cToken, janela,
                                  h.bindMemory(to: UInt8.self).baseAddress, hello.count,
                                  out_key, out_sessao, out_err)
                    }
                }
            }
        }
    }

    /// Reconexão (sem QR) com o código durável guardado no Keychain.
    static func reconectar(sala: String, codigo: String, hello: Data)
        throws -> (key: Data, sessao: SessaoTunel)
    {
        try executar(hello: hello) { out_key, out_sessao, out_err in
            sala.withCString { cSala in
                codigo.withCString { cCodigo in
                    hello.withUnsafeBytes { h in
                        dc_reconectar(cSala, cCodigo,
                                      h.bindMemory(to: UInt8.self).baseAddress, hello.count,
                                      out_key, out_sessao, out_err)
                    }
                }
            }
        }
    }

    private static func executar(
        hello: Data,
        _ chamada: (UnsafeMutablePointer<DcBytes>,
                    UnsafeMutablePointer<OpaquePointer?>,
                    UnsafeMutablePointer<DcBytes>) -> Int32
    ) throws -> (key: Data, sessao: SessaoTunel) {
        var key = DcBytes()
        var sessaoPtr: OpaquePointer?
        var erro = DcBytes()
        let rc = chamada(&key, &sessaoPtr, &erro)
        guard rc == DC_OK, let sp = sessaoPtr else {
            throw DotCrowError(mensagem: tomarErro(erro))
        }
        dc_bytes_free(erro)
        return (tomar(key), SessaoTunel(sp))
    }
}

/// As partes de um link `dotcrow://p?...`, lidas pelo Rust (mesma decodificação dos dois lados).
struct LinkPareamento {
    let relay: String
    let sala: String
    let token: String
    let janela: UInt64

    init?(uri: String) {
        var r = DcBytes(), s = DcBytes(), t = DcBytes(), e = DcBytes()
        var w: UInt64 = 0
        let rc = uri.withCString { dc_parse_pairing($0, &r, &s, &t, &w, &e) }
        guard rc == DC_OK else { dc_bytes_free(e); dc_bytes_free(r); dc_bytes_free(s); dc_bytes_free(t); return nil }
        dc_bytes_free(e)
        self.relay = String(data: tomar(r), encoding: .utf8) ?? ""
        self.sala = String(data: tomar(s), encoding: .utf8) ?? ""
        self.token = String(data: tomar(t), encoding: .utf8) ?? ""
        self.janela = w
        if relay.isEmpty || sala.isEmpty || token.isEmpty { return nil }
    }
}
