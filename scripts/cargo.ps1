# Executa cargo com os destinos Windows obrigatoriamente no D:.
# Uso: .\scripts\cargo.ps1 test --workspace
$ErrorActionPreference = 'Stop'
$env:TEMP = 'D:\tmp'
$env:TMP = 'D:\tmp'
$env:CARGO_TARGET_DIR = 'D:\cargo-target'
$env:CARGO_HOME = 'D:\.cargo'
$env:RUSTUP_HOME = 'D:\.rustup'
$env:GRADLE_USER_HOME = 'D:\.gradle'
$env:PIP_CACHE_DIR = 'D:\pip-cache'
$env:npm_config_cache = 'D:\npm-cache'
$env:UV_CACHE_DIR = 'D:\uv-cache'
$env:GOMODCACHE = 'D:\go\pkg\mod'
$env:GOCACHE = 'D:\go-build'

if (-not (Test-Path -LiteralPath $env:TEMP)) {
    New-Item -ItemType Directory -Path $env:TEMP | Out-Null
}
Push-Location (Split-Path -Parent $PSScriptRoot)
try {
    & cargo @args
    $cargoExitCode = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $cargoExitCode