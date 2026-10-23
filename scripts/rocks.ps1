$ErrorActionPreference='Stop'
$env:TEMP='D:\tmp'
$env:TMP='D:\tmp'
& wsl -d Ubuntu-24.04 -e bash /mnt/d/DEV/MachiavelliDB/scripts/rocks.sh @args
exit $LASTEXITCODE
