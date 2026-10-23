import SwiftUI

@main
struct DotCrowApp: App {
    @State private var modelo = AppModel()

    var body: some Scene {
        WindowGroup {
            RootView()
                .environment(modelo)
                .task { await modelo.aoAbrir() }
                .onOpenURL { url in
                    // O esquema dotcrow:// pode abrir o app direto com o link do QR.
                    Task { await modelo.lerQR(url.absoluteString) }
                }
        }
    }
}
