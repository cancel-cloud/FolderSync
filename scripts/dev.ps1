. (Join-Path $PSScriptRoot 'env.ps1')
Set-Location (Split-Path $PSScriptRoot -Parent)
pnpm install --frozen-lockfile
if ($LASTEXITCODE) { exit $LASTEXITCODE }
pnpm tauri dev
exit $LASTEXITCODE
