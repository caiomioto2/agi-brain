param([string]$BuildDirectory)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path -Parent $PSScriptRoot
if (-not $BuildDirectory) {
    $metadata = & (Join-Path $PSScriptRoot 'cargo.ps1') @('metadata','--format-version','1','--no-deps') | ConvertFrom-Json
    if ($LASTEXITCODE) { throw 'Cannot locate Cargo build directory' }
    $BuildDirectory = Join-Path $metadata.target_directory 'release'
}
$packagePath = Join-Path $workspace 'dist\AGI Brain'
New-Item -ItemType Directory -Force -Path $packagePath | Out-Null
foreach ($name in @('agi-brain.exe','agi-brain-context.exe')) {
    Copy-Item -LiteralPath (Join-Path $BuildDirectory $name) -Destination (Join-Path $packagePath $name) -Force
}
foreach ($name in @('README.md','LICENSE','VALIDATION.md')) {
    Copy-Item -LiteralPath (Join-Path $workspace $name) -Destination (Join-Path $packagePath $name) -Force
}
$docsPath = Join-Path $packagePath 'docs'
New-Item -ItemType Directory -Force -Path $docsPath | Out-Null
Copy-Item -LiteralPath (Join-Path $workspace 'docs\live-map-spec.md') -Destination $docsPath -Force
$readerPath = Join-Path $packagePath 'server'
New-Item -ItemType Directory -Force -Path $readerPath | Out-Null
Copy-Item -LiteralPath (Join-Path $workspace 'server\read_only_bridge.py') -Destination $readerPath -Force
Get-FileHash -LiteralPath (Join-Path $packagePath 'agi-brain.exe'), (Join-Path $packagePath 'agi-brain-context.exe') -Algorithm SHA256 |
    ForEach-Object { "$($_.Hash)  $([IO.Path]::GetFileName($_.Path))" } | Set-Content -LiteralPath (Join-Path $packagePath 'SHA256SUMS.txt') -Encoding utf8
Compress-Archive -Path (Join-Path $packagePath '*') -DestinationPath (Join-Path $workspace 'dist\AGI-Brain-Windows-x64.zip') -Force
Write-Output (Join-Path $workspace 'dist\AGI-Brain-Windows-x64.zip')
