# cargo fmt --all 적용 후 --check 검증.
# 인자 "check"이면 적용 없이 체크만.

$ErrorActionPreference = "Continue"
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
Set-Location $repo

if ($args.Count -ge 1 -and $args[0] -eq "check") {
  cargo fmt --all -- --check 2>&1 | Select-Object -Last 30
  exit $LASTEXITCODE
}

cargo fmt --all 2>&1 | Select-Object -Last 5
$applyExit = $LASTEXITCODE
if ($applyExit -ne 0) { exit $applyExit }

cargo fmt --all -- --check 2>&1 | Select-Object -Last 10
exit $LASTEXITCODE
