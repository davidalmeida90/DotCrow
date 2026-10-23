import SwiftUI

struct PairingView: View {
    @Environment(AppModel.self) private var modelo
    @State private var manual = ""
    @State private var mostrarManual = false

    var body: some View {
        VStack(spacing: 0) {
            QRScanner { uri in
                Task { await modelo.lerQR(uri) }
            }
            .ignoresSafeArea(edges: .top)
            .overlay(alignment: .center) {
                RoundedRectangle(cornerRadius: 24)
                    .stroke(.white.opacity(0.9), lineWidth: 3)
                    .frame(width: 240, height: 240)
            }
            .overlay(alignment: .bottom) {
                Text("Aponte para o QR no painel do DotCrow\n(ele troca a cada 10 segundos — tudo bem)")
                    .font(.footnote)
                    .multilineTextAlignment(.center)
                    .foregroundStyle(.white)
                    .padding(12)
                    .background(.black.opacity(0.5), in: RoundedRectangle(cornerRadius: 12))
                    .padding(.bottom, 24)
            }

            Button("Colar o link à mão") { mostrarManual = true }
                .padding()
        }
        .navigationTitle("Parear")
        .navigationBarTitleDisplayMode(.inline)
        .alert("Colar link", isPresented: $mostrarManual) {
            TextField("dotcrow://p?…", text: $manual)
            Button("Parear") { Task { await modelo.lerQR(manual) } }
            Button("Cancelar", role: .cancel) {}
        }
    }
}
