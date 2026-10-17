param(
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$NotesPath,
    [string]$OutputDirectory = 'target/release/bundle/nsis'
)
$ErrorActionPreference = 'Stop'
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Only stable semantic versions may be published as latest.' }
$folderSyncAssetName = "FolderSync_${Version}_x64-setup.exe"
$folderSyncAsset = Join-Path $OutputDirectory $folderSyncAssetName
$folderSyncSignature = [IO.File]::ReadAllText((Resolve-Path -LiteralPath "$folderSyncAsset.sig")).Trim()
$folderSyncDecodedSignature = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($folderSyncSignature))
if ($folderSyncDecodedSignature -notmatch "(?m)(?:^|`t)version:$([regex]::Escape($Version))(?:`r?$|`t)") { throw 'Updater signature is not bound to the release version.' }
$folderSyncManifest = [ordered]@{
    version = $Version
    notes = [IO.File]::ReadAllText((Resolve-Path -LiteralPath $NotesPath)).Replace("`r`n", "`n")
    pub_date = [DateTimeOffset]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ')
    platforms = @{ 'windows-x86_64' = @{
        signature = $folderSyncSignature
        url = "https://github.com/cancel-cloud/FolderSync/releases/download/v$Version/$folderSyncAssetName"
    } }
}
$folderSyncManifest | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'latest.json') -Encoding utf8NoBOM
$folderSyncHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $folderSyncAsset).Hash
"$folderSyncHash  $folderSyncAssetName" | Set-Content -LiteralPath (Join-Path $OutputDirectory "FolderSync_${Version}_SHA256SUMS.txt") -Encoding ascii
