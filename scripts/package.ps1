# Prépare dist\onde-<version>-windows-x64.zip (exécutable + README + exemple).
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$version = (Select-String -Path "$root\Cargo.toml" -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$stage = Join-Path $root "dist\onde-$version"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "$root\target\release\onde.exe" $stage
Copy-Item "$root\README.md" $stage
Copy-Item "$root\LICENSE" $stage
Copy-Item "$root\fonds\exemple.hlsl" $stage
$zip = Join-Path $root "dist\onde-$version-windows-x64.zip"
Remove-Item -Force $zip -ErrorAction SilentlyContinue
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Write-Host "Paquet : $zip"
