. (Join-Path $PSScriptRoot 'env.ps1')
Set-Location (Split-Path $PSScriptRoot -Parent)
pnpm tauri build
exit $LASTEXITCODE
