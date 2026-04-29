# cargo clippy --workspace --all-targets -- -D warnings 래퍼.
# PATH 보강 + 출력 트리밍 (마지막 30 + Finished/error/warning 라인만).
#
# 주의: cargo는 진행 상황을 stderr로 출력 — PowerShell이 NativeCommandError로 감싸지 않도록
# ErrorActionPreference는 Continue. 종료 코드는 cargo가 직접 결정.

$ErrorActionPreference = "Continue"
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
Set-Location $repo

cargo clippy --workspace --all-targets -- -D warnings 2>&1 |
  Select-String -Pattern "(error|warning|Finished)" |
  Select-Object -Last 30 |
  ForEach-Object { $_.ToString() }

exit $LASTEXITCODE
