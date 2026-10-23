import Foundation

/// O túnel do celular com o computador, por cima de um WebSocket com o relay.
///
/// A rede e a moldura dos frames ficam aqui, em Swift nativo (`URLSessionWebSocketTask`);
/// **toda a criptografia** vai para o Rust (`Handshake`, `SessaoTunel`). Assim o celular e o
/// computador falam byte a byte o mesmo protocolo (veja `docs/remoto.md`).
actor Tunnel {
    enum Estado: Equatable {
        case desligado
        case handshake
        case ligado
    }

    private let relay: String
    private let sala: String
    private var tarefa: URLSessionWebSocketTask?
    private var sessao: SessaoTunel?
    private var proximoId: Int = 0
    private(set) var estado: Estado = .desligado

    /// Credencial para o handshake: QR (1º pareamento) ou código durável (reconexão).
    enum Credencial {
        case qr(token: String, janela: UInt64)
        case codigo(String)
    }

    init(relay: String, sala: String) {
        self.relay = relay
        self.sala = sala
    }

    /// Conecta, faz o handshake e deixa o túnel pronto. No 1º pareamento devolve o
    /// **código durável** (frame `CRED`) para o app guardar no Keychain.
    @discardableResult
    func conectar(_ cred: Credencial) async throws -> String? {
        guard var url = URLComponents(string: relay) else {
            throw DotCrowError(mensagem: "endereço do relay inválido")
        }
        url.queryItems = [URLQueryItem(name: "sala", value: sala),
                          URLQueryItem(name: "rol", value: "guest")]
        guard let final = url.url else { throw DotCrowError(mensagem: "não montei a URL do relay") }

        let sessaoURL = URLSession(configuration: .default)
        let ws = sessaoURL.webSocketTask(with: final)
        ws.resume()
        tarefa = ws
        estado = .handshake

        // HI → HELLO
        try await enviar(Frame.montar(Tag.hi))
        let hello = try await receberFrame()
        guard hello.tag == Tag.hello else {
            throw DotCrowError(mensagem: "esperava HELLO, veio \(hello.tag)")
        }

        // KEY (handshake pelo Rust)
        let (key, sess): (Data, SessaoTunel)
        switch cred {
        case let .qr(token, janela):
            (key, sess) = try Handshake.parear(sala: sala, token: token, janela: janela, hello: hello.payload)
        case let .codigo(codigo):
            (key, sess) = try Handshake.reconectar(sala: sala, codigo: codigo, hello: hello.payload)
        }
        self.sessao = sess
        try await enviar(key)

        // OK
        let ok = try await receberFrame()
        if ok.tag == Tag.erro {
            throw DotCrowError(mensagem: String(data: ok.payload, encoding: .utf8) ?? "handshake recusado")
        }
        guard ok.tag == Tag.ok, (try? sess.decifrar(ok.payload)) != nil else {
            throw DotCrowError(mensagem: "não consegui abrir o OK do computador")
        }

        // CRED (só no 1º pareamento): o código durável, para reconectar depois sem QR.
        var codigoDuravel: String? = nil
        if case .qr = cred {
            let credFrame = try await receberFrame()
            if credFrame.tag == Tag.cred {
                let claro = try sess.decifrar(credFrame.payload)
                codigoDuravel = String(data: claro, encoding: .utf8)
            }
        }

        estado = .ligado
        return codigoDuravel
    }

    /// Uma requisição à API do painel (GET/POST), devolvendo o corpo da resposta.
    func requisitar(metodo: String, caminho: String, corpo: Any? = nil) async throws -> Data {
        guard let sessao, estado == .ligado else {
            throw DotCrowError(mensagem: "túnel não está ligado")
        }
        proximoId += 1
        let id = proximoId
        var pedido: [String: Any] = ["id": id, "metodo": metodo, "caminho": caminho]
        if let corpo { pedido["corpo"] = corpo }
        let json = try JSONSerialization.data(withJSONObject: pedido)
        try await enviar(sessao.cifrar(json))

        // Resposta: um frame DATA com o mesmo id.
        let resp = try await receberFrame()
        guard resp.tag == Tag.data else { throw DotCrowError(mensagem: "resposta fora do protocolo") }
        let claro = try sessao.decifrar(resp.payload)
        let obj = try JSONSerialization.jsonObject(with: claro) as? [String: Any]
        if let erro = obj?["erro"] as? String { throw DotCrowError(mensagem: erro) }
        if let corpo = obj?["corpo"] as? String { return Data(corpo.utf8) }
        return claro
    }

    /// Abre um fluxo SSE (ex.: os eventos ao vivo de uma tarefa). Cada `data:` do painel vira
    /// um `String` no `AsyncThrowingStream`, até o frame com `"fim": true`.
    func eventos(caminho: String) throws -> AsyncThrowingStream<String, Error> {
        guard let sessao, estado == .ligado else {
            throw DotCrowError(mensagem: "túnel não está ligado")
        }
        proximoId += 1
        let id = proximoId
        return AsyncThrowingStream { continuation in
            Task {
                do {
                    let pedido: [String: Any] = ["id": id, "metodo": "GET", "caminho": caminho, "sse": true]
                    let json = try JSONSerialization.data(withJSONObject: pedido)
                    try await enviar(sessao.cifrar(json))
                    while true {
                        let frame = try await receberFrame()
                        guard frame.tag == Tag.data else { continue }
                        let claro = try sessao.decifrar(frame.payload)
                        guard let obj = try JSONSerialization.jsonObject(with: claro) as? [String: Any] else { continue }
                        if obj["fim"] as? Bool == true { break }
                        if let pedaco = obj["sse"] as? String { continuation.yield(pedaco) }
                        if let erro = obj["erro"] as? String { throw DotCrowError(mensagem: erro) }
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: error)
                }
            }
        }
    }

    func desligar() {
        tarefa?.cancel(with: .normalClosure, reason: nil)
        tarefa = nil
        sessao = nil
        estado = .desligado
    }

    // MARK: - WebSocket cru

    private func enviar(_ bytes: Data) async throws {
        guard let tarefa else { throw DotCrowError(mensagem: "sem conexão") }
        try await tarefa.send(.data(bytes))
    }

    private func receberFrame() async throws -> (tag: UInt8, payload: Data) {
        guard let tarefa else { throw DotCrowError(mensagem: "sem conexão") }
        let msg = try await tarefa.receive()
        let bytes: Data
        switch msg {
        case let .data(d): bytes = d
        case let .string(s): bytes = Data(s.utf8)
        @unknown default: throw DotCrowError(mensagem: "mensagem de tipo desconhecido")
        }
        guard let frame = Frame.separar(bytes) else { throw DotCrowError(mensagem: "frame vazio") }
        return frame
    }
}
