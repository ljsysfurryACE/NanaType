# Build a release of NanaType and package it.
#   pwsh scripts/package.ps1 [-Portable] [-SkipBuild]
# Needs: nana-packager on PATH (cargo install --path <NanaUI>/crates/nana-packager),
# Inno Setup 6 (iscc) for the installer. Code signing is done in CI.
param([switch]$Portable, [switch]$SkipBuild)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"' | Select-Object -First 1).Matches.Groups[1].Value
if (-not $env:NANATYPE_BUILD_ID) { $env:NANATYPE_BUILD_ID = (git rev-parse --short HEAD) }
if (-not $SkipBuild) {
    $env:NANATYPE_REQUIRE_RESOURCES = "1"
    # Static C runtime: the package must not need the VC++ redistributable.
    $env:RUSTFLAGS = "-C target-feature=+crt-static"
    cargo build --profile dist -p nanatype
}
$exe = Join-Path $root "target\dist\nanatype.exe"

# DX12 shader compiler shipped beside the exe (MIT-licensed DXC). Set DXC_DLL
# to an official release build; otherwise the Windows SDK copy is used.
$dxc = $env:DXC_DLL
if (-not $dxc) {
    $dxc = Get-ChildItem "C:\Program Files (x86)\Windows Kits\10\bin\*\x64\dxcompiler.dll" |
        Sort-Object FullName | Select-Object -Last 1 -ExpandProperty FullName
}
New-Item -ItemType Directory -Force (Join-Path $root "target\dxc") | Out-Null
Copy-Item $dxc (Join-Path $root "target\dxc\dxcompiler.dll") -Force
# Outside target/: NanaUI treats executables under a cargo target dir as dev builds.
$out = Join-Path $root "dist"
Remove-Item -Recurse -Force $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $out | Out-Null

$config = Join-Path $root "packaging\nana-package.toml"
if ($Portable) {
    $portable = Join-Path $out "nana-package.toml"
    (Get-Content $config) -replace 'backend = "installer"', 'backend = "portable"' | Set-Content $portable
    $config = $portable
}
nana-packager package --config $config --exe $exe --out $out --build-id $env:NANATYPE_BUILD_ID
$app = Join-Path $out "app\NanaType"
nana-packager validate $app --run

if ($Portable) {
    Compress-Archive -Path (Join-Path $app "*") -DestinationPath (Join-Path $out "NanaType-$version-portable.zip")
} else {
    iscc "/DAppVersion=$version" "/DSourceDir=$app" "/DOutputDir=$out" (Join-Path $root "packaging\nanatype.iss")
}
Write-Host "Packaged NanaType $version into $out"
