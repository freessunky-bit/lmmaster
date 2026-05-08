# Phase 8'.c.4 — API key UX 개편 + LAN 게이트웨이 노출 결정 노트

> 작성일: 2026-05-09
> 상태: 확정 (사용자 승인 — "정식 (A)로 구현 가자")
> 선행: ADR-0007 (게이트웨이 보안 모델), ADR-0022 (게이트웨이 라우팅 + Scope), ADR-0029 (per-key Pipelines override)
> 후행: Phase 8'.c.5 (LAN audit 분리, v1.x), Phase 20' Connect Mode (v2.0+ 보류)

## 0. 결정 요약 (7가지)

1. **Phase 8'.c.4 신설** — Phase 8'.c.3 (ADR-0029 per-key Pipelines) 자연 후속. API 키 UX + LAN 노출을 한 페이즈에 묶음 (라디오가 LAN 토글 진입 trigger).
2. **`Scope.network_scope: Option<NetworkScope>` 신규** — `"localhost" | "lan" | "any"`. `serde(default)` + `skip_serializing_if`로 기존 키 호환.
3. **"어디서 호출?" 라디오 = Origin 입력 치환** — Origin URL 직접 입력 → 의도 기반 3-way 라디오. 사용자 멘탈모델 일치.
4. **Origin 필수 해제** — `network_scope` 라디오만으로 발급 가능. Origin은 "고급 설정" 안으로.
5. **모델 multi-select** — 설치된 모델 체크박스 + "전체" sentinel. glob은 고급 설정.
6. **Reveal step "이렇게 쓰세요"** — base URL / 헤더 / 모델 ID / curl 예시 동적 생성. `network_scope`에 따라 LAN URL 노출.
7. **`UserSettings.gateway_allow_external` persistence + Settings 토글** — `0.0.0.0` 바인딩 활성화. 변경 시 게이트웨이 재시작 권유.

## 1. 채택안

### 1.1 Phase 번호 — 8'.c.4 (Phase 8'.c lineage)

- ADR-0022 (Phase 8'.c.1): Gateway routing + Scope.
- ADR-0028 (Phase 8'.c.2): Pipelines hot-reload.
- ADR-0029 (Phase 8'.c.3): Per-key Pipelines override.
- **ADR-0066 (Phase 8'.c.4): LAN exposure + API key UX overhaul** ← 본 페이즈.

후속 8'.c.5는 v1.x — LAN 호출의 audit log 분리 (현재 ring buffer 전역).

### 1.2 데이터 모델 — `network_scope` 명시 저장

```rust
// crates/key-manager/src/scope.rs
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkScope {
    Localhost,  // 127.0.0.1 only
    Lan,        // localhost + private LAN ranges
    Any,        // 사용자가 외부 터널을 직접 띄움 (LMmaster는 키만 발급)
}

pub struct Scope {
    // ... 기존 필드 ...

    /// Phase 8'.c.4 — 이 키가 어디서 호출될 의도인지.
    /// `None` = 호환 모드 (기존 키 = Localhost로 해석).
    /// `Some(Localhost)` = 127.0.0.1 only — strict.
    /// `Some(Lan)` = 127.0.0.1 + private LAN ranges (10/8, 172.16/12, 192.168/16).
    /// `Some(Any)` = origin 검증만, 외부 터널 사용 — LMmaster가 노출 책임 X.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_scope: Option<NetworkScope>,
}
```

ADR-0029 패턴 그대로 — `serde(default)` + JSON column 통째 — DB 마이그레이션 0.

**enforcement**: `auth::require_api_key`에서 요청 source IP (`ConnectInfo<SocketAddr>`)를 `network_scope`와 매칭. `Localhost` 키에 LAN IP 호출 → 401.

### 1.3 UI — ApiKeyIssueModal 개편 (4 part)

**Part 1 — 별칭** (기존 그대로)
- `aliasLabel` / `aliasPlaceholder` 카피만 미세 조정.

**Part 2 — "어디서 호출할 거예요?" 라디오 (신규, Origin 치환)**
```
⦿ 이 PC만 (127.0.0.1) — 동일 PC만, 가장 안전
⚪ 사내망 (같은 WiFi/유선) — 동료 PC, 사내 폰까지
   └ allow_external 꺼져 있으면 [켜러 가기 →] 인라인 링크
⚪ 어디서나 (외부 터널 직접 셋업) — cloudflared/ngrok 등
   └ "LMmaster는 키만 발급, 터널은 사용자가 직접" 안내
```
- 라디오 선택 = `network_scope` 직접 매핑.
- 사내망 선택 + allow_external=false → 발급 버튼 옆 "사내망 노출 설정 후 발급" 안내. 키 발급은 막지 않음 (의도는 보존).
- "어디서나" 선택 시 경고 카피 노출 ("LMmaster 외부에서 키 노출 책임은 사용자에게 있어요").

**Part 3 — 허용 모델 multi-select (glob 텍스트 → 체크박스 리스트)**
- `listInstalledModels` 신규 IPC — 카탈로그 entries 중 *현재 설치된* 것만 반환. Catalog.tsx의 dropdown filter와 동일 패턴 (`list_local_llama_cpp_models` 참고).
- "전체 (앞으로 설치할 모델도 자동 허용)" 토글 = `models: ["*"]` sentinel.
- 개별 체크 = 모델 ID 배열 (`models: ["qwen-3-30b-a3b", ...]`).
- 빈 선택 시 발급 거부 ("모델을 1개 이상 선택해 주세요").

**Part 4 — 고급 설정 collapse (default 접힘)**
- Origin 직접 입력 (기존 `allowed_origins`) — power user / 정확 origin 매칭 필요시.
- 경로 패턴 (기존 `endpoints`).
- 만료 시각 (기존 `expires_at`).
- 키별 필터 (기존 `enabled_pipelines`, ADR-0029).

99%는 default로 발급 가능 + 1%는 고급 펼쳐서 정밀 제어.

### 1.4 Reveal step — "이렇게 쓰세요" 동적 생성

```
sk_lmm_xxxxxxxx... [복사]    ⏱ 6초 뒤 마스크

── 이렇게 쓰세요 ──────────────
Base URL
  이 PC      http://127.0.0.1:8788/v1   [복사]
  사내망     http://192.168.1.42:8788/v1 [복사] (network_scope=lan일 때만)
헤더        Authorization: Bearer <키>
모델 ID 예  qwen-3-30b-a3b              [복사]

curl 예시                                [전체 복사]
curl http://192.168.1.42:8788/v1/chat/completions \
  -H "Authorization: Bearer ..." \
  -H "Content-Type: application/json" \
  -d '{"model":"qwen-3-30b-a3b","messages":[...]}'
```

- `network_scope=localhost` → 127.0.0.1만.
- `network_scope=lan` → 127.0.0.1 + 자동 감지된 첫 LAN IP.
- `network_scope=any` → 127.0.0.1 + "외부 URL은 사용자가 터널 셋업 후 직접 사용" 안내.
- 키 평문은 8초 후 마스크되지만 **호출 가이드 섹션은 닫을 때까지 노출** (사용자가 옮겨 적을 시간 필요).

### 1.5 Settings — 사내망 노출 토글

```rust
// apps/desktop/src-tauri/src/settings/mod.rs
pub struct UserSettings {
    pub llama_server_path: Option<String>,
    /// Phase 8'.c.4 — 게이트웨이 0.0.0.0 바인딩 허용.
    /// 변경 후 게이트웨이 재시작 필요.
    #[serde(default)]
    pub gateway_allow_external: bool,
}
```

- `apply_startup_env`에서 `LMMASTER_GATEWAY_ALLOW_EXTERNAL` env 주입 → `GatewayConfig::from_env`가 읽음.
- 토글 변경 시 `set_gateway_allow_external(bool)` IPC → settings.json save + env 갱신 + 사용자에게 "재시작 후 적용" 안내.
- 자동 게이트웨이 hot-restart는 v1.x (현재는 사용자가 앱 재시작).

LAN IP 표시:
- `local-ip-address` crate (workspace.dependencies 추가) — Win/Mac/Linux 동일 API.
- private 범위 화이트리스트 (10/8, 172.16/12, 192.168/16, 169.254/16 link-local 제외).
- IPC `list_lan_addresses() -> Vec<String>` — 다중 NIC 환경 모두 노출.

### 1.6 ApiKeysPanel — 카드 보강

- 키 카드에 `network_scope` 뱃지: 🏠 "이 PC만" / 🏢 "사내망" / 🌐 "어디서나".
  - **주의**: CLAUDE.md §4.3 "이모지 직접 사용 금지" → lucide-react 아이콘 사용 (Home / Building / Globe).
- LAN URL quick copy: 사내망 키에서 한 클릭 LAN URL 복사.
- ApiKeyEditModal도 동일 라디오 통합 (편집 시 network_scope 변경 가능).

### 1.7 i18n 카피 톤 (CLAUDE.md §4.1)

- 라디오 라벨: "이 PC만", "사내망 (같은 WiFi/유선)", "어디서나 (외부 터널)".
- 토글: "사내망의 다른 기기가 호출하게 할게요".
- 경고: "회사 PC면 보안팀에 먼저 상의해 주세요" (해요체 + 책임 환기).
- 발급 버튼: "발급할게요" (기존 유지).

ko.json + en.json 동시 갱신. fallback 깨짐 방지.

## 2. 기각안 + 이유 (Negative space)

### 2.1 LMmaster 자체에 cloudflared launcher 통합

- **검토**: `network_scope=any` 선택 시 LMmaster가 cloudflared 다운로드 + spawn + 공개 URL 표시.
- **거부**: ADR-0055 "외부 통신 0" + ADR-0007 "localhost-only 바인딩" 정신 위반. SaaS(Cloudflare) 의존이 LMmaster의 thesis 깨뜨림. 사용자가 자기 PC에서 별도 도구로 띄우는 건 LMmaster 외부 행위 — OK. LMmaster가 띄우는 건 정체성 위반.
- **재검토 트리거**: ADR-0058 Connect Mode v2.0 진입 시 "사용자 소유 mesh"의 정의에 self-hosted relay가 들어가면 재검토.

### 2.2 Origin 필드 완전 삭제

- **검토**: "어디서 호출?" 라디오만으로 충분. allowed_origins 필드 제거.
- **거부**: power user는 *정확 origin 매칭* 필요 (예: `https://my-blog.com`만 허용, 다른 사이트 X). 라디오는 의도 표현, origin은 정밀 enforcement — 두 axis가 직교. 고급 설정 안에 보존.
- **재검토 트리거**: 사용자 데이터 분석상 99.9%가 라디오만으로 충분하고 origin 입력 사례가 0이면 v2.0에 삭제 검토.

### 2.3 자동 hot-restart (allow_external 토글 시)

- **검토**: Settings 토글 변경 즉시 게이트웨이 stop → 새 config로 listen → React state 갱신.
- **거부**: 게이트웨이 lifecycle은 현재 startup-only. hot-restart 시 in-flight SSE 스트림 / 진행 중 요청 처리가 복잡 (graceful shutdown + reconnect). v1.x로 이월 — 현재는 "재시작 후 적용" 안내가 충분.
- **재검토 트리거**: 사용자 베타 피드백상 "재시작 귀찮음"이 top 3 안에 들어오면 v1.x에서 우선 처리.

### 2.4 CIDR 화이트리스트 (특정 사내망 IP 범위만)

- **검토**: `network_scope=lan` 대신 `allowed_cidrs: Vec<String>` 명시 (예: `192.168.1.0/24`).
- **거부**: 복잡도 대비 가치 적음. private 범위(RFC 1918) 전체 허용이 90% 사용자 시나리오 충족. 정밀 제어 필요하면 origin 검증으로 대체. CIDR 파싱 + IP 비교 + 다중 NIC 환경 처리는 3배 코드 비용.
- **재검토 트리거**: 엔터프라이즈 사용자 발견 시 (현재 v1 thesis는 개인/소규모 팀).

### 2.5 모델 multi-select 대신 검색 가능한 dropdown

- **검토**: 50+ 모델 설치한 power user 위해 search input + 결과 리스트.
- **거부**: 현재 사용자 PC 평균 3~10개 모델. 체크박스 리스트가 mental load 더 낮음. 50+ 사용자가 발견되면 v1.x에 search 추가.
- **재검토 트리거**: 카탈로그 entries 100+ 도달 시 (현재 42).

### 2.6 라디오 "어디서나" 대신 "외부" + 별도 터널 셋업 마법사

- **검토**: 외부 노출은 별도 마법사로 빼서 "키 발급 → 마법사 진입 → cloudflared 가이드".
- **거부**: 키 발급 흐름이 두 갈래로 분리되면 사용자 인지 부담. 라디오 선택 + 안내 텍스트 + 가이드 링크가 단순. 마법사는 *Settings → 외부 노출 가이드* 별도 진입로로 (v1.x).
- **재검토 트리거**: 사용자 베타에서 "라디오 '어디서나' 선택 후 뭐 해야 하는지 모르겠다" 피드백 다수.

## 3. 미정 / 후순위 이월

| 항목 | 이월 사유 | 진입 조건 / 페이즈 |
|---|---|---|
| 게이트웨이 hot-restart (toggle 즉시 반영) | in-flight SSE / 요청 graceful shutdown 복잡 | v1.x 사용자 피드백 trigger |
| LAN audit log 분리 (현재 ring buffer 전역) | ring buffer 전역 그루핑 X — 새 audit schema 필요 | Phase 8'.c.5 (v1.x) |
| Connect Mode (PC ↔ PC 페어링) | ADR-0058 — v2.0+ 보류, 본 페이즈와 별 트랙 | Phase 20' v2.0+ |
| LAN URL QR 코드 | 동료 폰 셋업 편의, 본 페이즈는 핵심 흐름 우선 | sub-phase 5 (옵션) |
| CIDR 화이트리스트 | 엔터프라이즈 시나리오 (현재 v1 개인 thesis) | v2.0+ |
| 자동 cloudflared launcher | "외부 통신 0" 정책 분기 — ADR 신설 필요 | ADR-0058 v2.0+ |
| 키별 검색 가능 모델 dropdown | 카탈로그 entries 100+ 도달 시 | v1.x |
| API 키 메타 export (CSV/JSON) | 베타 사용자 다중 PC 운용 시 가치 | v1.x |

## 4. 테스트 invariant

### Backend (Rust)
- **`Scope::network_scope` round-trip**: 4 variants (`null` / `localhost` / `lan` / `any`) 직렬화/역직렬화 + 기존 키(필드 부재) → `None` deserialize.
- **`UserSettings::gateway_allow_external` persistence**: save + load round-trip + missing field default false.
- **LAN IP 감지**: 다중 NIC 환경 mock + 루프백 제외 + private 범위 화이트리스트.
- **`auth::require_api_key` enforcement**: `network_scope=Localhost` 키에 LAN IP 호출 → 401, `network_scope=Lan` 키에 같은 호출 → 200, `network_scope=Any` 키에 어떤 IP든 origin만 검증.
- **GatewayConfig env**: `LMMASTER_GATEWAY_ALLOW_EXTERNAL=1` → bind_host = 0.0.0.0.

### Frontend (React + vitest)
- **a11y**: vitest-axe — 라디오 그룹, 모달, multi-select 모두 violations 0.
- **라디오 ↔ network_scope 매핑**: 3 라디오 클릭 → state.network_scope 정확히 변경.
- **사내망 라디오 + allow_external=false**: "켜러 가기" 인라인 링크 노출 + 클릭 시 Settings 라우팅.
- **모델 multi-select**:
  - "전체" 토글 ON → 개별 체크박스 disable + state.models = ["*"].
  - "전체" 토글 OFF → 개별 체크박스 활성 + 빈 선택 시 발급 거부.
- **Reveal step base URL 분기**:
  - `network_scope=localhost` → 127.0.0.1만 노출, LAN URL 미노출.
  - `network_scope=lan` + LAN IP 감지 성공 → 두 URL 모두 노출.
  - `network_scope=any` → 127.0.0.1 + "외부 URL은 사용자 셋업" 안내.
- **curl 예시 생성**: 모델 ID 첫 번째 선택값 + 현재 base URL + 키 prefix(masked) 정확 삽입.
- **i18n**: ko/en 1:1 매핑, fallback 깨짐 0.
- **scoped 쿼리**: `within()` / `data-testid` — 동일 텍스트 다중 출현 시 정확 단언.

### 한국어 카피 invariant
- 라디오/토글 라벨 모두 해요체 (CLAUDE.md §4.1).
- "회사 PC면 보안팀에 먼저 상의해 주세요" 경고 카피 정확 출현 (Settings 토글 + 발급 모달 사내망 라디오).
- "발급할게요" / "취소" / "복사할게요" 기존 카피 유지.

## 5. 다음 페이즈 인계

### 선행 의존성 (이미 충족)
- ADR-0022 (Phase 8'.c.1) — Gateway routing + Scope.
- ADR-0029 (Phase 8'.c.3) — `serde(default)` 호환 패턴 확립.
- `crates/core-gateway/src/config.rs::GatewayConfig::allow_external` — 데이터 모델 이미 존재.

### 이 페이즈 산출물

**ADR**: `docs/adr/0066-lan-gateway-exposure-and-api-key-ux.md`

**Backend**:
- `crates/key-manager/src/scope.rs` — `NetworkScope` enum + `Scope.network_scope` 필드.
- `crates/core-gateway/src/auth.rs` — `network_scope` enforcement (LAN IP 매칭).
- `apps/desktop/src-tauri/src/settings/mod.rs` — `gateway_allow_external` persistence + env 주입.
- `apps/desktop/src-tauri/src/gateway/commands.rs` (또는 신설) — `set_gateway_allow_external`, `list_lan_addresses` IPC.
- `apps/desktop/src-tauri/src/keys/commands.rs` — `list_installed_models` IPC.
- `Cargo.toml` workspace.dependencies — `local-ip-address` 추가.

**Frontend**:
- `apps/desktop/src/components/keys/ApiKeyIssueModal.tsx` — 라디오 + multi-select + 고급 collapse 개편.
- `apps/desktop/src/components/keys/ApiKeyRevealStep.tsx` — 신규 분리 + "이렇게 쓰세요" 가이드.
- `apps/desktop/src/components/keys/ApiKeyEditModal.tsx` — 라디오 통합.
- `apps/desktop/src/components/keys/ApiKeysPanel.tsx` — 카드 뱃지 + LAN quick copy.
- `apps/desktop/src/pages/Settings.tsx` — 사내망 노출 섹션.
- `apps/desktop/src/ipc/keys.ts` — `Scope.network_scope` 타입 + `listInstalledModels` helper.
- `apps/desktop/src/ipc/settings.ts` (또는 신설) — `setGatewayAllowExternal`, `listLanAddresses`.
- `apps/desktop/src/i18n/ko.json` + `en.json` — `keys.modal.network.*`, `settings.gateway.lan.*` 신규 ~30 키.

**테스트**:
- `crates/key-manager/src/scope.rs` 내 `network_scope` round-trip 테스트.
- `apps/desktop/src/components/keys/ApiKeyIssueModal.test.tsx` 갱신 — 라디오/multi-select.
- `apps/desktop/src/components/keys/ApiKeyRevealStep.test.tsx` 신규 — base URL 분기.
- `apps/desktop/src/pages/Settings.test.tsx` 갱신 — 토글 + 경고 + LAN IP 표시.

### Sub-phase 분할 (PR 단위)
1. **Sub-phase 1** — Settings 사내망 노출 토글 + LAN IP 감지 + persistence.
2. **Sub-phase 2** — ApiKeyIssueModal 라디오 + multi-select + 고급 collapse + Scope.network_scope 데이터 모델.
3. **Sub-phase 3** — RevealStep 분리 + "이렇게 쓰세요" 가이드.
4. **Sub-phase 4** — ApiKeysPanel 카드 뱃지 + ApiKeyEditModal 통합 + LAN quick copy.

각 sub-phase 끝에 RESUME 갱신 + 테스트 카운트 차분 명시.

### 위험 노트

- **`local-ip-address` crate 호환성**: Windows + Linux + macOS 모두 지원하는지 확인. 회사 PC가 Windows면 winapi 의존 없이 동작해야 함 (CLAUDE.md "외부 통신 0" + portable runtime).
- **Tauri sidecar gateway 재시작 hook**: 현재 가용 여부 미확인. 없으면 사용자 앱 재시작으로 우회.
- **기존 키 호환**: `network_scope=None` → 기본 enforcement는 *기존 동작* 유지 (allowed_origins만 검사). 새 필드 추가가 기존 키 동작 변경 X.
- **Origin 검증 + network_scope 둘 다 통과해야 호출 OK**: 의도가 직교라 AND 결합. 사용자 멘탈모델 잘 안내해야 (라디오 + Origin이 모두 통과해야 한다는 점).
- **회사 PC 보안 경고 카피**: 베타 사용자가 회사 PC에서 무심코 켜는 시나리오 차단. 발급 모달 + Settings 토글 양쪽에 경고 노출.
- **LAN IP 감지 빈 결과**: VPN 단독 / 가상 어댑터만 있는 환경에서 LAN IP가 없을 수 있음. 빈 배열 graceful + "사내망 IP를 감지하지 못했어요" 카피.

## 6. 참고

### 글로벌 사례
- **Ollama OLLAMA_HOST=0.0.0.0**: LAN 노출 패턴의 반면교사 — 인증 0이라 보안 사고 사례. LMmaster는 키 인증 + network_scope enforcement로 보강.
- **OpenAI/Anthropic API**: 키 발급 시 origin/IP 검증 X — 서버사이드 사용 가정. LMmaster는 브라우저-first라 정책 다름.
- **Stripe Restricted Keys**: 키별 권한 화이트리스트 패턴 — `network_scope`가 유사한 의도 표현.
- **GitHub Personal Access Token (Fine-grained)**: 라디오로 scope 표현 + 고급 설정 collapse — 본 페이즈 UX 직접 차용.

### 관련 ADR
- ADR-0007: 게이트웨이 보안 모델 + 키 인증.
- ADR-0022: Gateway routing + Scope schema.
- ADR-0028: Pipelines hot-reload.
- ADR-0029: Per-key Pipelines override.
- ADR-0055: 네트워크 정책 강화.
- ADR-0058: Connect Mode (v2.0+ 보류, 본 페이즈와 별 트랙).
- ADR-0066: 본 페이즈 (LAN exposure + API key UX).

### 메모리 항목
- 신규 — `phase_8pc4_lan_api_ux.md`: Phase 8'.c.4 진행 + Sub-phase 분할 + 회사 PC 시나리오 안전 가이드.

### 관련 라이브러리
- `local-ip-address` 0.6+ (MIT/Apache-2, no-std friendly, Win/Mac/Linux).
- `lucide-react` (이미 도입, CLAUDE.md §4.3) — Home / Building / Globe 아이콘.

---

**Phase 진입 조건 충족** ✅ — sub-phase 1부터 즉시 시작 가능.
