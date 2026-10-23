$ErrorActionPreference = 'Stop'
$env:TEMP = 'D:\tmp'
$env:TMP = 'D:\tmp'
$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $repoRoot.StartsWith('D:\', [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'As provas e artefatos devem ficar no D:.'
}
$linuxScript = '/mnt/d/' + $repoRoot.Substring(3).Replace('\', '/') + '/scripts/lean.sh'
if ($args.Count -eq 0) {
    & wsl -d Ubuntu-24.04 -e bash $linuxScript build
} else {
    & wsl -d Ubuntu-24.04 -e bash $linuxScript @args
}
exit $LASTEXITCODE
