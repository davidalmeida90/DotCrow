import Foundation

/// Os marcadores de cada frame do túnel (o 1º byte), iguais aos do `dotcrow-tunel`.
enum Tag {
    static let hi: UInt8 = 0x01     // celular → PC: cheguei
    static let hello: UInt8 = 0x02  // PC → celular: identidade pública (em claro)
    static let key: UInt8 = 0x03    // celular → PC: envelope pós-quântico {credencial, K}
    static let ok: UInt8 = 0x04     // PC → celular: {"ok":true} cifrado
    static let cred: UInt8 = 0x05   // PC → celular: código durável cifrado (só no 1º pareamento)
    static let data: UInt8 = 0x10   // ambos: requisição/resposta JSON cifrada
    static let erro: UInt8 = 0x1f   // PC → celular: erro de handshake em claro
}

enum Frame {
    /// Monta `tag || payload`.
    static func montar(_ tag: UInt8, _ payload: Data = Data()) -> Data {
        var d = Data([tag])
        d.append(payload)
        return d
    }

    /// Separa `(tag, payload)` de um frame recebido.
    static func separar(_ frame: Data) -> (tag: UInt8, payload: Data)? {
        guard let primeiro = frame.first else { return nil }
        return (primeiro, Data(frame.dropFirst()))
    }
}
