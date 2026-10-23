import Foundation
import Observation

/// Um corvo (ou faixa da equipe) como o painel mostra: nome e o que está pensando/fazendo.
struct Corvo: Identifiable, Hashable {
    let id: String
    var nome: String
    var papel: String
    var pensando: String
}

/// O estado do app: pareamento, conexão do túnel e os corvos ao vivo.
@MainActor
@Observable
final class AppModel {
    enum Fase: Equatable {
        case carregando
        case semPar          // precisa ler o QR
        case conectando
        case pronto
        case erro(String)
    }

    var fase: Fase = .carregando
    var corvos: [Corvo] = []
    var mensagemTarefa: String = ""

    private var tunnel: Tunnel?
    private var tarefaAtual: String?

    /// Na abertura: se já houver pareamento guardado, reconecta sem QR.
    func aoAbrir() async {
        if let par = Cofre.carregar() {
            await conectar(relay: par.relay, sala: par.sala, cred: .codigo(par.codigo))
        } else {
            fase = .semPar
        }
    }

    /// Chamado quando o usuário lê um QR `dotcrow://p?...`.
    func lerQR(_ uri: String) async {
        guard let link = LinkPareamento(uri: uri) else {
            fase = .erro("QR não é um link de pareamento do DotCrow")
            return
        }
        await conectar(relay: link.relay, sala: link.sala,
                       cred: .qr(token: link.token, janela: link.janela))
    }

    private func conectar(relay: String, sala: String, cred: Tunnel.Credencial) async {
        fase = .conectando
        let t = Tunnel(relay: relay, sala: sala)
        do {
            let codigoNovo = try await t.conectar(cred)
            if let codigoNovo {
                Cofre.salvar(relay: relay, sala: sala, codigo: codigoNovo)
            }
            self.tunnel = t
            fase = .pronto
            await atualizarCorvos()
        } catch {
            fase = .erro(error.localizedDescription)
        }
    }

    /// Lê o estado do painel e monta a lista de corvos.
    func atualizarCorvos() async {
        guard let tunnel else { return }
        do {
            let dados = try await tunnel.requisitar(metodo: "GET", caminho: "/api/estado")
            if let obj = try JSONSerialization.jsonObject(with: dados) as? [String: Any] {
                corvos = Self.corvosDoEstado(obj)
            }
        } catch {
            fase = .erro(error.localizedDescription)
        }
    }

    /// Manda uma tarefa nova para o bando.
    func enviarTarefa(_ texto: String) async {
        guard let tunnel, !texto.isEmpty else { return }
        do {
            let resp = try await tunnel.requisitar(metodo: "POST", caminho: "/api/tarefas",
                                                   corpo: ["text": texto])
            if let obj = try JSONSerialization.jsonObject(with: resp) as? [String: Any],
               let id = obj["id"] as? String {
                tarefaAtual = id
                await acompanhar(id)
            }
        } catch {
            fase = .erro(error.localizedDescription)
        }
    }

    /// Acompanha os eventos ao vivo de uma tarefa (SSE pelo túnel) e atualiza os corvos.
    func acompanhar(_ id: String) async {
        guard let tunnel else { return }
        do {
            let fluxo = try await tunnel.eventos(caminho: "/api/tarefas/\(id)/eventos")
            for try await pedaco in fluxo {
                for linha in pedaco.split(separator: "\n") where linha.hasPrefix("data:") {
                    let json = linha.dropFirst("data:".count).trimmingCharacters(in: .whitespaces)
                    if let d = json.data(using: .utf8),
                       let ev = try? JSONSerialization.jsonObject(with: d) as? [String: Any] {
                        aplicarEvento(ev)
                    }
                }
            }
        } catch {
            fase = .erro(error.localizedDescription)
        }
    }

    /// Manda uma mensagem para a tarefa em andamento (a caixa de conversa ao vivo).
    func conversar(_ texto: String) async {
        guard let tunnel, let id = tarefaAtual, !texto.isEmpty else { return }
        _ = try? await tunnel.requisitar(metodo: "POST",
                                         caminho: "/api/tarefas/\(id)/mensagem",
                                         corpo: ["text": texto])
    }

    func desparear() {
        Cofre.esquecer()
        Task { await tunnel?.desligar() }
        tunnel = nil
        corvos = []
        fase = .semPar
    }

    // MARK: - Tradução dos eventos do painel

    private func aplicarEvento(_ ev: [String: Any]) {
        guard let tipo = ev["type"] as? String else { return }
        switch tipo {
        case "lane_event", "stage_delta":
            if let faixa = ev["lane"] as? String ?? ev["corvo"] as? String {
                let texto = (ev["delta"] as? String) ?? (ev["text"] as? String) ?? ""
                if let i = corvos.firstIndex(where: { $0.id == faixa }) {
                    corvos[i].pensando += texto
                } else {
                    corvos.append(Corvo(id: faixa, nome: faixa, papel: "", pensando: texto))
                }
            }
        case "owner_message":
            mensagemTarefa = (ev["text"] as? String) ?? mensagemTarefa
        default:
            break
        }
    }

    private static func corvosDoEstado(_ obj: [String: Any]) -> [Corvo] {
        guard let lista = obj["corvos"] as? [[String: Any]] else { return [] }
        return lista.map { c in
            Corvo(id: (c["id"] as? String) ?? UUID().uuidString,
                  nome: (c["nome"] as? String) ?? "corvo",
                  papel: (c["papel"] as? String) ?? "",
                  pensando: (c["pensando"] as? String) ?? "")
        }
    }
}
