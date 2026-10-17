. (Join-Path $PSScriptRoot 'env.ps1')
Set-Location (Split-Path $PSScriptRoot -Parent)
# Updater signing is independent of Windows Authenticode. Keep this key outside Git.
if (-not $env:TAURI_SIGNING_PRIVATE_KEY) {
    $folderSyncKeyDirectory = Join-Path $env:USERPROFILE '.codex/keys/FolderSync'
    $folderSyncKeyFile = Join-Path $folderSyncKeyDirectory 'updater.key'
    $folderSyncPasswordFile = Join-Path $folderSyncKeyDirectory 'updater.password'
    if (-not (Test-Path -LiteralPath $folderSyncKeyFile)) { throw 'Set TAURI_SIGNING_PRIVATE_KEY to the FolderSync updater signing key before packaging.' }
    $env:TAURI_SIGNING_PRIVATE_KEY = $folderSyncKeyFile
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [IO.File]::ReadAllText($folderSyncPasswordFile)
}
pnpm tauri build
exit $LASTEXITCODE
