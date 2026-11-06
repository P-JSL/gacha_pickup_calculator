# This file never contains the PFX/password. Both are passed from GitHub Secrets
# only on main push releases; use a trusted Authenticode provider certificate.
$ErrorActionPreference = 'Stop'
$exe = Join-Path (Get-Location) 'target/release/gacha_pickup_calculator.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    throw 'The Windows release executable was not built.'
}
$hasCert = -not [string]::IsNullOrWhiteSpace($env:WIN_SIGN_PFX_BASE64)
$hasPassword = -not [string]::IsNullOrWhiteSpace($env:WIN_SIGN_PFX_PASSWORD)
if (-not $hasCert -and -not $hasPassword) {
    Write-Host '::warning title=Unsigned Windows executable::No trusted Authenticode certificate configured; Sigstore provenance is not Authenticode.'
    exit 0
}
if (-not $hasCert -or -not $hasPassword) {
    throw 'Both WINDOWS_SIGNING_PFX_BASE64 and WINDOWS_SIGNING_PFX_PASSWORD must be configured.'
}
$kitRoot = Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Windows Kits/10/bin'
$signtool = Get-ChildItem -LiteralPath $kitRoot -Filter signtool.exe -Recurse -File -ErrorAction Stop |
    Where-Object { $_.FullName -match '[\\/]x64[\\/]signtool\.exe$' } |
    Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) { throw 'Trusted Windows SDK signtool.exe (x64) is unavailable.' }
$pfx = Join-Path $env:RUNNER_TEMP 'gacha-release-signing.pfx'
try {
    [System.IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WIN_SIGN_PFX_BASE64))
    & $signtool.FullName sign /fd SHA256 /tr https://timestamp.digicert.com /td SHA256 /f $pfx /p $env:WIN_SIGN_PFX_PASSWORD $exe
    if ($LASTEXITCODE -ne 0) { throw "Authenticode signing failed: exit $LASTEXITCODE" }
    & $signtool.FullName verify /pa /all $exe
    if ($LASTEXITCODE -ne 0) { throw "Authenticode verification failed: exit $LASTEXITCODE" }
    Write-Host 'Windows executable signed and Authenticode validation succeeded.'
} finally {
    Remove-Item -LiteralPath $pfx -Force -ErrorAction SilentlyContinue
}
