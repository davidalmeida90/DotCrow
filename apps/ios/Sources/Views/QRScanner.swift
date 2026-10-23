import SwiftUI
import AVFoundation

/// Câmera que lê QR code, embrulhada para o SwiftUI. Chama `aoLer` com o texto do 1º QR.
struct QRScanner: UIViewControllerRepresentable {
    var aoLer: (String) -> Void

    func makeCoordinator() -> Coordenador { Coordenador(aoLer: aoLer) }

    func makeUIViewController(context: Context) -> ScannerVC {
        let vc = ScannerVC()
        vc.delegate = context.coordinator
        return vc
    }

    func updateUIViewController(_ vc: ScannerVC, context: Context) {}

    final class Coordenador: NSObject, AVCaptureMetadataOutputObjectsDelegate {
        let aoLer: (String) -> Void
        private var jaLeu = false

        init(aoLer: @escaping (String) -> Void) { self.aoLer = aoLer }

        func metadataOutput(_ output: AVCaptureMetadataOutput,
                            didOutput objetos: [AVMetadataObject],
                            from connection: AVCaptureConnection) {
            guard !jaLeu,
                  let obj = objetos.first as? AVMetadataMachineReadableCodeObject,
                  let texto = obj.stringValue else { return }
            jaLeu = true
            aoLer(texto)
        }
    }
}

/// Um `UIViewController` simples que roda a sessão de captura e mostra a prévia.
final class ScannerVC: UIViewController {
    weak var delegate: AVCaptureMetadataOutputObjectsDelegate?
    private let sessao = AVCaptureSession()
    private var camada: AVCaptureVideoPreviewLayer?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black
        guard let camera = AVCaptureDevice.default(for: .video),
              let entrada = try? AVCaptureDeviceInput(device: camera),
              sessao.canAddInput(entrada) else { return }
        sessao.addInput(entrada)

        let saida = AVCaptureMetadataOutput()
        guard sessao.canAddOutput(saida) else { return }
        sessao.addOutput(saida)
        saida.setMetadataObjectsDelegate(delegate, queue: .main)
        saida.metadataObjectTypes = [.qr]

        let camada = AVCaptureVideoPreviewLayer(session: sessao)
        camada.videoGravity = .resizeAspectFill
        camada.frame = view.layer.bounds
        view.layer.addSublayer(camada)
        self.camada = camada
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        if !sessao.isRunning {
            DispatchQueue.global(qos: .userInitiated).async { [sessao] in sessao.startRunning() }
        }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        if sessao.isRunning { sessao.stopRunning() }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        camada?.frame = view.layer.bounds
    }
}
