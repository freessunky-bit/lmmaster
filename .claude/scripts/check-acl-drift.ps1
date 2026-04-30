# Tauri ACL drift 검증 — invoke_handler에 등록된 명령이 capabilities/main.json에 모두 있는지 확인.
#
# 정책 (phase-install-bench-bugfix-decision §6 — 리서치 보강):
# - lib.rs의 `invoke_handler![commands::xxx, module::yyy, ...]` 블록을 파싱해 명령 목록 추출.
# - permissions/*.toml의 `commands.allow = [...]`에서 등록된 명령 모두 추출.
# - capabilities/main.json의 permissions 배열에서 사용된 identifier 추출 + 그 identifier들이 어떤 commands를 가리키는지 매핑.
# - invoke_handler에는 있는데 capabilities를 통해 도달 가능한 커맨드 집합에 없는 항목을 fail.
#
# 사용:
#   .\.claude\scripts\check-acl-drift.ps1
# 또는 verify.ps1에 통합돼 자동 실행.

$ErrorActionPreference = "Stop"

$repo = (Resolve-Path "$PSScriptRoot\..\..").Path
$lib = "$repo\apps\desktop\src-tauri\src\lib.rs"
$permsDir = "$repo\apps\desktop\src-tauri\permissions"
$capsFile = "$repo\apps\desktop\src-tauri\capabilities\main.json"

# 1) lib.rs의 invoke_handler 블록에서 명령 추출.
$libContent = Get-Content $lib -Raw
if ($libContent -notmatch "invoke_handler\(tauri::generate_handler!\[([\s\S]*?)\]\)") {
  Write-Error "ACL DRIFT CHECK FAIL: lib.rs에서 invoke_handler 블록을 찾지 못했어요."
  exit 1
}
$handlerBlock = $Matches[1]

# 각 줄의 마지막 segment(`module::path::command_name` 또는 `command_name`)에서 함수 이름 추출.
$handlerCommands = @()
foreach ($line in $handlerBlock -split "`n") {
  $trimmed = $line.Trim().TrimEnd(',').Trim()
  if ($trimmed -eq "" -or $trimmed.StartsWith("//")) { continue }
  $tokens = $trimmed -split '::'
  $cmd = $tokens[-1].Trim()
  if ($cmd -ne "") { $handlerCommands += $cmd }
}
$handlerCommands = $handlerCommands | Sort-Object -Unique

# 2) permissions/*.toml에서 identifier ↔ commands 매핑 추출.
$identifierToCommands = @{}
Get-ChildItem $permsDir -Filter "*.toml" | ForEach-Object {
  $content = Get-Content $_.FullName -Raw
  # [[permission]] 블록을 split.
  $blocks = $content -split '(?=\[\[permission\]\])' | Where-Object { $_ -match '\[\[permission\]\]' }
  foreach ($block in $blocks) {
    if ($block -match 'identifier\s*=\s*"([^"]+)"' ) {
      $id = $Matches[1]
      if ($block -match 'commands\.allow\s*=\s*\[([^\]]+)\]') {
        $cmdList = $Matches[1]
        $cmds = ([regex]::Matches($cmdList, '"([^"]+)"') | ForEach-Object { $_.Groups[1].Value })
        $identifierToCommands[$id] = @($cmds)
      }
    }
  }
}

# 3) capabilities/main.json의 permissions 배열에서 사용된 identifier 추출.
$capsRaw = Get-Content $capsFile -Raw | ConvertFrom-Json
$capIdentifiers = @()
foreach ($p in $capsRaw.permissions) {
  if ($p -is [string]) {
    $capIdentifiers += $p
  } elseif ($p.identifier) {
    $capIdentifiers += $p.identifier
  }
}

# 4) capabilities를 통해 노출되는 명령 집합 계산.
$allowedCommands = @()
foreach ($id in $capIdentifiers) {
  if ($identifierToCommands.ContainsKey($id)) {
    $allowedCommands += $identifierToCommands[$id]
  }
}
$allowedCommands = $allowedCommands | Sort-Object -Unique

# 5) 차이 계산. 현재는 모든 app-defined 명령이 명시 ACL을 가져야 한다 — 이전에 가정했던
#    "ping / get_gateway_status auto-allow"는 사실이 아니었음 (사용자 첫 화면이 "booting" stuck
#    되던 root cause). whitelist 제거 + 모두 명시 등록.
$missing = @()
foreach ($cmd in $handlerCommands) {
  if ($cmd -notin $allowedCommands) { $missing += $cmd }
}

# 7) 결과 보고.
Write-Output ""
Write-Output "=== Tauri ACL drift check ==="
Write-Output "invoke_handler!: $($handlerCommands.Count) 개 명령"
Write-Output "capabilities/main.json: $($capIdentifiers.Count) identifier (앱-defined $($allowedCommands.Count) 개 명령 매핑)"

if ($missing.Count -gt 0) {
  Write-Output ""
  Write-Output "FAIL: 다음 명령이 ACL에 등록되지 않았어요. 사용자가 호출하면 'not allowed: not found' 에러로 막혀요:"
  foreach ($m in $missing) { Write-Output "  - $m" }
  Write-Output ""
  Write-Output "수정: permissions/<관련>.toml에 [[permission]] 블록 추가 + capabilities/main.json permissions 배열에 identifier 추가."
  exit 1
}

Write-Output "OK: 모든 명령이 capabilities에서 도달 가능해요."
exit 0
