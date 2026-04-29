# pnpm run build (tsc -b + vite build) — 풀 production 빌드.

$ErrorActionPreference = "Continue"
$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
Set-Location "$repo\apps\desktop"

pnpm run build 2>&1 | Select-Object -Last 20
exit $LASTEXITCODE
