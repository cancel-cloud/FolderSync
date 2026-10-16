. (Join-Path $PSScriptRoot 'env.ps1')
Set-Location (Split-Path $PSScriptRoot -Parent)
cargo build -p foldersync-engine --example benchmark
if ($LASTEXITCODE) { exit $LASTEXITCODE }
$reportDirectory = Join-Path (Get-Location) 'docs\product\benchmarks'
New-Item -ItemType Directory -Force $reportDirectory | Out-Null
$benchmarkProcess = Start-Process -FilePath (Join-Path (Get-Location) 'target\debug\examples\benchmark.exe') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $reportDirectory 'latest.json') -RedirectStandardError (Join-Path $reportDirectory 'latest.stderr.txt')
$peakWorkingSet = 0L
while (-not $benchmarkProcess.HasExited) {
    $benchmarkProcess.Refresh()
    $peakWorkingSet = [Math]::Max($peakWorkingSet, $benchmarkProcess.PeakWorkingSet64)
    Start-Sleep -Milliseconds 250
}
@{ peak_working_set_bytes = $peakWorkingSet; exit_code = $benchmarkProcess.ExitCode } | ConvertTo-Json | Set-Content (Join-Path $reportDirectory 'latest-memory.json')
Get-Content (Join-Path $reportDirectory 'latest.json')
exit $benchmarkProcess.ExitCode
