$ErrorActionPreference = 'Stop'
$cargoDirectory = Join-Path $env:USERPROFILE '.cargo\bin'
if (Test-Path $cargoDirectory) { $env:PATH = "$cargoDirectory;$env:PATH" }
if (-not (Get-Command node -ErrorAction SilentlyContinue)) {
    $bundledNode = Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin'
    if (Test-Path $bundledNode) { $env:PATH = "$bundledNode;$env:PATH" }
}
if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
    $bundledPnpm = Join-Path $env:USERPROFILE '.cache\codex-runtimes\codex-primary-runtime\dependencies\bin\fallback'
    if (Test-Path $bundledPnpm) { $env:PATH = "$bundledPnpm;$env:PATH" }
}
# Use Windows' trusted certificate store; TLS verification remains enabled.
$env:NODE_OPTIONS = '--use-system-ca'
