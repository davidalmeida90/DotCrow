# Instalador do DotCrow para Windows (sem precisar de Rust).
#
# Baixa a versão mais nova do dotcrow.exe publicada no GitHub, coloca em
#   %LOCALAPPDATA%\Programs\DotCrow, adiciona ao PATH do seu usuário e cria
#   um atalho no Menu Iniciar. Não precisa de administrador.
#
# Para rodar, abra o PowerShell e cole:
#   irm https://raw.githubusercontent.com/napoleaomp4/dotcrow/main/packaging/windows/instalar.ps1 | iex
#
# Prefere um instalador de clicar? Baixe o dotcrow-setup.exe na página de
# Releases: https://github.com/napoleaomp4/dotcrow/releases

$ErrorActionPreference = "Stop"
$repo = "napoleaomp4/dotcrow"
$dest = Join-Path $env:LOCALAPPDATA "Programs\DotCrow"

function Info($m) { Write-Host "▸ $m" -ForegroundColor Cyan }
function Ok($m)   { Write-Host "✔ $m" -ForegroundColor Green }

Info "Procurando a versão mais nova do DotCrow..."
$headers = @{ "User-Agent" = "dotcrow-instalar" }
try {
  $release = Invoke-RestMethod -Headers $headers "https://api.github.com/repos/$repo/releases/latest"
} catch {
  throw "Não achei nenhuma versão publicada ainda em https://github.com/$repo/releases. Assim que sair a primeira release, rode de novo."
}
$asset = $release.assets | Where-Object { $_.name -eq "dotcrow.exe" } | Select-Object -First 1
if (-not $asset) { throw "A release $($release.tag_name) não tem o dotcrow.exe. Use o dotcrow-setup.exe na página de Releases." }

New-Item -ItemType Directory -Force -Path $dest | Out-Null
$exe = Join-Path $dest "dotcrow.exe"
Info "Baixando $($release.tag_name)..."
Invoke-WebRequest -Headers $headers -Uri $asset.browser_download_url -OutFile $exe

# Adiciona ao PATH do usuário, se ainda não estiver.
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if (($userPath -split ";") -notcontains $dest) {
  [Environment]::SetEnvironmentVariable("Path", "$userPath;$dest", "User")
  $env:Path = "$env:Path;$dest"
  Ok "DotCrow adicionado ao PATH (abra um terminal novo para valer)."
}

# Atalho no Menu Iniciar que abre o painel.
$startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\DotCrow"
New-Item -ItemType Directory -Force -Path $startMenu | Out-Null
$lnk = (New-Object -ComObject WScript.Shell).CreateShortcut((Join-Path $startMenu "DotCrow.lnk"))
$lnk.TargetPath = $exe
$lnk.WindowStyle = 7  # minimizada: o painel abre no navegador
$lnk.IconLocation = $exe
$lnk.Save()

$versao = (& $exe --version)
Ok "$versao instalado em $dest"
Write-Host ""
Write-Host "  Abra um terminal novo e rode:"
Write-Host "    dotcrow painel        # painel visual (chaves, tarefas, corvos)"
Write-Host "    dotcrow testar        # testa suas chaves de API"
Write-Host "    dotcrow --help        # todos os comandos"
