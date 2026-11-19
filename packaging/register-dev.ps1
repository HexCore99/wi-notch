param([switch] $StageOnly)

$ErrorActionPreference = 'Stop'
$projectDirectory = Split-Path -Parent $PSScriptRoot
$packageDirectory = Join-Path $projectDirectory 'target/notifications-dev'
$assetDirectory = Join-Path $packageDirectory 'Assets'

Push-Location -LiteralPath $projectDirectory
try {
    $rustTarget = rustc --version --verbose | Where-Object { $_ -eq 'host: x86_64-pc-windows-msvc' }
    if (!$rustTarget) { throw 'Use the x64 Windows MSVC Rust toolchain for this package.' }
    cargo build --locked --target-dir target
    if ($LASTEXITCODE -ne 0) { throw 'The wi-notch build failed.' }
} finally {
    Pop-Location
}

New-Item -ItemType Directory -Path $assetDirectory -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectDirectory 'target/debug/wi-notch.exe') -Destination $packageDirectory -Force
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'AppxManifest.xml') -Destination $packageDirectory -Force

Add-Type -AssemblyName System.Drawing
foreach ($logo in @(@{ Name = 'StoreLogo.png'; Size = 50 }, @{ Name = 'Logo150.png'; Size = 150 }, @{ Name = 'Logo44.png'; Size = 44 })) {
    $size = $logo.Size
    $bitmap = [System.Drawing.Bitmap]::new($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([System.Drawing.Color]::FromArgb(27, 34, 48))
        $graphics.FillRectangle([System.Drawing.Brushes]::WhiteSmoke, [int]($size * 0.2), [int]($size * 0.4), [int]($size * 0.6), [int]($size * 0.2))
        $bitmap.Save((Join-Path $assetDirectory $logo.Name), [System.Drawing.Imaging.ImageFormat]::Png)
    } finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

$manifest = Join-Path $packageDirectory 'AppxManifest.xml'
if (!$StageOnly) {
    Add-AppxPackage -Register $manifest
    Write-Output 'Launch wi-notch (Development) from the Start menu.'
} else {
    Write-Output $manifest
}
