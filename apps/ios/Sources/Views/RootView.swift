import SwiftUI

struct RootView: View {
    @Environment(AppModel.self) private var modelo

    var body: some View {
        NavigationStack {
            switch modelo.fase {
            case .carregando, .conectando:
                VStack(spacing: 16) {
                    ProgressView()
                    Text(modelo.fase == .conectando ? "Ligando o túnel…" : "Abrindo…")
                        .foregroundStyle(.secondary)
                }
            case .semPar:
                PairingView()
            case .pronto:
                TasksView()
            case let .erro(msg):
                VStack(spacing: 16) {
                    Image(systemName: "exclamationmark.triangle")
                        .font(.largeTitle)
                    Text(msg).multilineTextAlignment(.center).padding(.horizontal)
                    Button("Ler o QR de novo") { modelo.desparear() }
                        .buttonStyle(.borderedProminent)
                }
            }
        }
    }
}
