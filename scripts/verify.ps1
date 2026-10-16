. (Join-Path $PSScriptRoot 'env.ps1')
Set-Location (Split-Path $PSScriptRoot -Parent)
cargo fmt --all -- --check
if ($LASTEXITCODE) { exit $LASTEXITCODE }
cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE) { exit $LASTEXITCODE }
cargo test --workspace
if ($LASTEXITCODE) { exit $LASTEXITCODE }
pnpm typecheck
if ($LASTEXITCODE) { exit $LASTEXITCODE }
pnpm lint
if ($LASTEXITCODE) { exit $LASTEXITCODE }
pnpm test
if ($LASTEXITCODE) { exit $LASTEXITCODE }
pnpm build
exit $LASTEXITCODE
