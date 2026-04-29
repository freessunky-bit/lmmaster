# pnpm exec tsc -b — TypeScript 빌드 체크.

$ErrorActionPreference = "Continue"
$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
Set-Location "$repo\apps\desktop"

pnpm exec tsc -b 2>&1 | Select-Object -Last 30
exit $LASTEXITCODE
