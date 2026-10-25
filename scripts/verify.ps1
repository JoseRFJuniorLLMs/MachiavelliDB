# Verificação reproduzível da alpha. Todos os artefatos Windows ficam no D:.
[CmdletBinding()]
param([switch]$RocksDB,[switch]$Lean,[switch]$Qualification)
$ErrorActionPreference='Stop'
$env:TEMP='D:\tmp'; $env:TMP='D:\tmp'
$repoRoot=Split-Path -Parent $PSScriptRoot
if (-not $repoRoot.StartsWith('D:\',[StringComparison]::OrdinalIgnoreCase)) { throw 'Execute a cópia do projeto no D:.' }
$reportRoot=Join-Path 'D:\tmp' ('machiavelli-verify-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $reportRoot | Out-Null
$results=[Collections.Generic.List[object]]::new()
function Invoke-Gate([string]$Name,[scriptblock]$Command) {
    Write-Host "Validando $Name..."
    $timer=[Diagnostics.Stopwatch]::StartNew()
    & $Command 2>&1 | Tee-Object -FilePath (Join-Path $reportRoot ($Name+'.log')) | Out-Host
    $code=$LASTEXITCODE
    $results.Add([pscustomobject]@{gate=$Name;exit_code=$code;seconds=[Math]::Round($timer.Elapsed.TotalSeconds,3)})
    if ($code -ne 0) { throw "$Name falhou (código $code)." }
}
Push-Location $repoRoot
try {
    Invoke-Gate 'format' { & "$PSScriptRoot\cargo.ps1" fmt --all '--' --check }
    Invoke-Gate 'tests' { & "$PSScriptRoot\cargo.ps1" test --workspace --locked --offline }
    Invoke-Gate 'clippy' { & "$PSScriptRoot\cargo.ps1" clippy --workspace --all-targets --locked --offline '--' -D warnings }
    Invoke-Gate 'build' { & "$PSScriptRoot\cargo.ps1" build --workspace --locked --offline }
    Invoke-Gate 'smoke' { & "$PSScriptRoot\smoke-api.ps1" }
    if($RocksDB){Invoke-Gate 'rocksdb' { & "$PSScriptRoot\rocks.ps1" test -p machiavelli-storage -p machiavelli-centaur -p machiavelli-fog --features machiavelli-storage/rocksdb-backend --locked --offline }}
    if($Lean){Invoke-Gate 'lean' { & "$PSScriptRoot\lean.ps1" build }}
    if($Qualification){
        Invoke-Gate 'cfr-qualification' { & "$PSScriptRoot\cargo.ps1" run -p machiavelli-cfr --example cfr-qualification --locked --offline }
        Invoke-Gate 'cow-qualification' { & "$PSScriptRoot\cargo.ps1" run -p machiavelli-cow-arena --example cow-arena-qualification --locked --offline }
    }
    $results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $reportRoot 'results.json') -Encoding utf8
    Write-Output "PASS: todos os gates solicitados. Evidências: $reportRoot"
} catch {
    $results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $reportRoot 'results.json') -Encoding utf8
    Write-Error "Verificação interrompida: $_. Evidências: $reportRoot" -ErrorAction Continue
    exit 1
} finally { Pop-Location }
