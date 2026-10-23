import SwiftUI

struct TasksView: View {
    @Environment(AppModel.self) private var modelo
    @State private var pedido = ""
    @State private var conversa = ""

    var body: some View {
        VStack(spacing: 0) {
            if modelo.corvos.isEmpty {
                ContentUnavailableView("Nenhum corvo trabalhando",
                                       systemImage: "bird",
                                       description: Text("Mande uma tarefa abaixo e acompanhe o bando ao vivo."))
                    .frame(maxHeight: .infinity)
            } else {
                List(modelo.corvos) { corvo in
                    VStack(alignment: .leading, spacing: 4) {
                        HStack {
                            Text(corvo.nome).font(.headline)
                            if !corvo.papel.isEmpty {
                                Text(corvo.papel).font(.caption).foregroundStyle(.secondary)
                            }
                        }
                        Text(corvo.pensando.isEmpty ? "…" : corvo.pensando)
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 4)
                }
                .listStyle(.plain)
            }

            if !modelo.mensagemTarefa.isEmpty {
                Text(modelo.mensagemTarefa)
                    .font(.footnote)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                    .background(.yellow.opacity(0.15))
            }

            Divider()
            HStack {
                TextField("Falar com o bando agora…", text: $conversa)
                    .textFieldStyle(.roundedBorder)
                Button {
                    Task { await modelo.conversar(conversa); conversa = "" }
                } label: { Image(systemName: "bubble.right") }
                    .disabled(conversa.isEmpty)
            }
            .padding(.horizontal)

            HStack {
                TextField("Nova tarefa para os corvos…", text: $pedido)
                    .textFieldStyle(.roundedBorder)
                Button {
                    Task { await modelo.enviarTarefa(pedido); pedido = "" }
                } label: { Image(systemName: "paperplane.fill") }
                    .disabled(pedido.isEmpty)
            }
            .padding([.horizontal, .bottom])
        }
        .navigationTitle("DotCrow")
        .toolbar {
            ToolbarItem(placement: .topBarTrailing) {
                Menu {
                    Button("Atualizar") { Task { await modelo.atualizarCorvos() } }
                    Button("Desparear este celular", role: .destructive) { modelo.desparear() }
                } label: { Image(systemName: "ellipsis.circle") }
            }
        }
    }
}
