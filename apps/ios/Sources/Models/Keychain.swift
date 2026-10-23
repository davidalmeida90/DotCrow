import Foundation
import Security

/// Guarda o **código durável** (o segredo que reconecta sem QR) no Keychain, e o
/// relay/sala em claro no `UserDefaults` (não são segredos). O código durável é o que
/// precisa de proteção, então fica no Keychain com `ThisDeviceOnly`.
enum Cofre {
    private static let servico = "si.dotcrow.app"
    private static let conta = "codigo-duravel"

    struct Pareado {
        let relay: String
        let sala: String
        let codigo: String
    }

    /// O pareamento guardado, se houver.
    static func carregar() -> Pareado? {
        let d = UserDefaults.standard
        guard let relay = d.string(forKey: "relay"),
              let sala = d.string(forKey: "sala"),
              let codigo = lerCodigo() else { return nil }
        return Pareado(relay: relay, sala: sala, codigo: codigo)
    }

    static func salvar(relay: String, sala: String, codigo: String) {
        let d = UserDefaults.standard
        d.set(relay, forKey: "relay")
        d.set(sala, forKey: "sala")
        gravarCodigo(codigo)
    }

    static func esquecer() {
        let d = UserDefaults.standard
        d.removeObject(forKey: "relay")
        d.removeObject(forKey: "sala")
        SecItemDelete(baseQuery() as CFDictionary)
    }

    // MARK: - Keychain

    private static func baseQuery() -> [String: Any] {
        [kSecClass as String: kSecClassGenericPassword,
         kSecAttrService as String: servico,
         kSecAttrAccount as String: conta]
    }

    private static func gravarCodigo(_ codigo: String) {
        SecItemDelete(baseQuery() as CFDictionary)
        var q = baseQuery()
        q[kSecValueData as String] = Data(codigo.utf8)
        q[kSecAttrAccessible as String] = kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        SecItemAdd(q as CFDictionary, nil)
    }

    private static func lerCodigo() -> String? {
        var q = baseQuery()
        q[kSecReturnData as String] = true
        q[kSecMatchLimit as String] = kSecMatchLimitOne
        var item: CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary, &item) == errSecSuccess,
              let d = item as? Data else { return nil }
        return String(data: d, encoding: .utf8)
    }
}
