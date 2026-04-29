# cargo test --workspace 래퍼. 테스트 카운트만 추출.
# 전 binaries 결과를 한 줄씩 + 합계 표시.

$ErrorActionPreference = "Continue"
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
Set-Location $repo

$results = cargo test --workspace 2>&1 |
  Select-String "test result:" |
  Where-Object { $_ -notmatch "0 passed" } |
  ForEach-Object { $_.ToString() }

$results | ForEach-Object { Write-Output $_ }

$total = 0
foreach ($line in $results) {
  if ($line -match "(\d+) passed") {
    $total += [int]$Matches[1]
  }
}
Write-Output "----"
Write-Output "TOTAL: $total passed"

exit $LASTEXITCODE
