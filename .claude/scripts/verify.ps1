# 풀 검증 — 매 sub-phase 종료 직전 호출.
# 순서: cargo fmt --check → cargo clippy → cargo test → tsc -b → vite build.
# 한 단계라도 실패하면 즉시 종료 + 비-zero exit code.

$ErrorActionPreference = "Continue"
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
Set-Location $repo

function Step($name, $block) {
  Write-Output ""
  Write-Output "=== $name ==="
  & $block
  if ($LASTEXITCODE -ne 0) {
    Write-Output ""
    Write-Output "FAIL: $name (exit $LASTEXITCODE)"
    exit $LASTEXITCODE
  }
  Write-Output "OK: $name"
}

Step "Tauri ACL drift" {
  # IPC 명령이 capabilities/main.json에 모두 등록됐는지 확인 — 누락 시 사용자 클릭 실패.
  & "$PSScriptRoot\check-acl-drift.ps1"
}

Step "cargo fmt --check" {
  cargo fmt --all -- --check 2>&1 | Select-Object -Last 10
}

Step "cargo clippy --workspace --all-targets -D warnings" {
  cargo clippy --workspace --all-targets -- -D warnings 2>&1 |
    Select-String -Pattern "(error|warning|Finished)" |
    Select-Object -Last 15 |
    ForEach-Object { $_.ToString() }
}

Step "cargo test --workspace" {
  $results = cargo test --workspace 2>&1 |
    Select-String "test result:" |
    Where-Object { $_ -notmatch "0 passed" } |
    ForEach-Object { $_.ToString() }
  $results | ForEach-Object { Write-Output $_ }
  $total = 0
  foreach ($line in $results) {
    if ($line -match "(\d+) passed") { $total += [int]$Matches[1] }
  }
  Write-Output "TOTAL: $total passed"
}

Step "tsc -b" {
  Push-Location "$repo\apps\desktop"
  pnpm exec tsc -b 2>&1 | Select-Object -Last 15
  Pop-Location
}

Step "vite build" {
  Push-Location "$repo\apps\desktop"
  pnpm run build 2>&1 | Select-Object -Last 8
  Pop-Location
}

Write-Output ""
Write-Output "ALL VERIFICATION STEPS PASSED"
exit 0
