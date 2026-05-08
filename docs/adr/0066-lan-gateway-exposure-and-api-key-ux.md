# ADR-0066: LAN 게이트웨이 노출 + API 키 UX 개편

- Status: Accepted
- Date: 2026-05-09
- Related: ADR-0007 (게이트웨이 보안 모델), ADR-0022 (Gateway routing + Scope), ADR-0029 (Per-key Pipelines override), ADR-0055 (네트워크 정책 강화), ADR-0058 (Connect Mode v2.0+ 보류)
- Phase: 8'.c.4
- Decision note: `docs/research/phase-8pc4-api-key-ux-lan-gateway-decision.md`

## Context

사용자 베타 테스트 시나리오: 본인이 LMmaster를 띄우고 발급한 API 키를 자신의 웹앱에 연동해 *외부 사용자*가 5G 등으로 접근해 테스트하기를 원함. 그러나 v1 `127.0.0.1` only 바인딩 + 기존 발급 모달의 origin URL 입력 방식이 사용자 멘탈모델과 어긋남:

1. **localhost-only 바인딩** — 같은 PC 외에는 도달 불가. LAN의 동료 폰조차 못 옴 (단, `GatewayConfig.allow_external` 데이터 모델은 이미 존재, UI 미노출).
2. **Origin URL 입력 강제** — OpenAI/Claude 키처럼 "그냥 발급" 기대인데 origin 1+ 필수. 개발 중인 웹앱은 URL 미정 시나리오 차단.
3. **모델 패턴 glob 텍스트** — 사용자가 모델 ID 정규식 외워야 함. "내가 설치한 모델 중 어떤 것을 허용할지" 의도와 어긋남.
4. **발급 후 호출 가이드 부재** — 키 평문만 보여주고 닫음. base URL / 헤더 / 모델 ID / curl 예시 어디에도 없음.
5. **키별 필터(ADR-0029) 인지 부담** — 첫 키 발급에서부터 4개 체크박스 + 전역 vs override 라는 2-axis 멘탈모델 부담.

5G/공공 인터넷 노출은 ADR-0055 "외부 통신 0" + ADR-0058 Connect Mode 보류 정책으로 LMmaster가 자동화하지 않음 (사용자가 별도 cloudflared 등 직접 운용). 그러나 **사내망(LAN) 노출**은 *제3자 SaaS 의존이 아닌 사용자 소유 네트워크 내부 통신*이라 정책 정신 위반 X — 별 모드로 명시 토글.

## Decision

### Schema — `Scope.network_scope` 신규

```rust
// crates/key-manager/src/scope.rs
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum NetworkScope {
    Localhost,
    Lan,
    Any,
}

pub struct Scope {
    // ... 기존 필드 ...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network_scope: Option<NetworkScope>,
}
```

- `None` (default, 기존 키) → `Localhost` 동작으로 해석.
- `Some(Localhost)` → 127.0.0.1 호출만 허용.
- `Some(Lan)` → 127.0.0.1 + private LAN 범위(RFC 1918) 호출 허용.
- `Some(Any)` → IP 검증 안 함, origin 검증만 (사용자가 외부 터널을 직접 띄우는 시나리오).

### Migration

ADR-0029 패턴 그대로 — `serde(default, skip_serializing_if)` + JSON column 통째 보관. DB 마이그레이션 0. `PRAGMA user_version` 변경 불필요.

### Wire

1. **`KeyManager::verify`**: `Scope.network_scope`를 `AuthOutcome`에 포함 (또는 extension에 주입).
2. **`auth::require_api_key`**: 요청의 source IP (`ConnectInfo<SocketAddr>`)를 `network_scope`와 매칭:
   - `Localhost`: 127.0.0.1 / ::1만 통과.
   - `Lan`: localhost + 10.0.0.0/8 + 172.16.0.0/12 + 192.168.0.0/16 통과.
   - `Any`: IP 검증 skip (origin 검증만).
3. **GatewayConfig**: `allow_external = true`일 때 `0.0.0.0` 바인딩 (이미 구현됨).
4. **UserSettings**: `gateway_allow_external: bool` persistence + startup env 주입.

### UI

**ApiKeyIssueModal — 4 part 재구성**
1. 별칭 (기존).
2. **"어디서 호출할 거예요?" 라디오** (신규, Origin URL 치환):
   - 이 PC만 / 사내망 / 어디서나 — `network_scope` 직접 매핑.
   - 사내망 + allow_external=false → 인라인 "켜러 가기" 링크.
3. **허용 모델 multi-select** (glob 텍스트 → 체크박스):
   - "전체" sentinel (`models: ["*"]`) + 설치된 모델 개별 체크박스.
   - 빈 선택 거부.
4. **고급 설정 collapse** (default 접힘):
   - Origin 직접 입력 (정확 매칭 필요 시).
   - 경로 패턴 / 만료 시각 / 키별 필터 (ADR-0029).

**Reveal step — "이렇게 쓰세요" 동적 가이드**
- 키 평문 + 8초 자동 마스크 (기존 정책).
- 추가: Base URL / 헤더 / 모델 ID 예시 / curl 예시. `network_scope`에 따라 LAN URL 동적 노출.
- 호출 가이드 섹션은 닫을 때까지 노출 (마스크되지 않음).

**Settings — 사내망 노출 토글**
- 별 섹션. `gateway_allow_external` 토글.
- LAN IP 자동 감지 (`local-ip-address` crate) — 다중 NIC 모두 노출.
- 변경 시 "재시작 후 적용" 안내. 자동 hot-restart는 v1.x.

**ApiKeysPanel — 카드 보강**
- `network_scope` 뱃지 (lucide-react Home / Building / Globe).
- LAN URL quick copy.
- ApiKeyEditModal도 동일 라디오 통합.

### 보안 경고 카피

- 발급 모달 사내망 라디오 + Settings 토글 양쪽에 "회사 PC면 보안팀에 먼저 상의해 주세요".
- 어디서나 라디오에 "LMmaster는 키만 발급, 외부 터널 셋업/노출 책임은 사용자에게 있어요".

## Consequences

### 긍정

- **사용자 멘탈모델 일치**: "어디서 호출?" 라디오가 origin URL 입력보다 의도 표현에 적합. OpenAI/GitHub PAT 패턴.
- **5G 시나리오 우회로 명확화**: LMmaster 정책 안에서는 LAN까지. 외부는 사용자 책임 (cloudflared 등). thesis 보존.
- **DB 마이그레이션 0**: ADR-0029 패턴 재사용 — JSON column + `serde(default)`.
- **회사 PC 안전 카피**: 베타 사용자 보안 사고 사전 차단.
- **호출 가이드 동봉**: 발급 후 "이제 어떻게 써요?" 사용자 질문 즉시 해소.
- **모델 multi-select**: glob 정규식 학습 부담 0. 카탈로그 ↔ 키 발급 일관성.

### 부정

- **2-axis 멘탈모델**: `network_scope` (라디오) + `allowed_origins` (고급) 두 axis 직교. UI 안내가 정확해야 (Origin 미입력 시 wildcard origin 동작).
- **`local-ip-address` 신규 의존**: workspace.dependencies +1. 단 가벼운 crate (libc only).
- **`network_scope=Any` 의도 모호**: "외부 터널 사용"이지만 IP 검증 X = 누구든 통과. 사용자가 origin 검증을 함께 거는지가 보안 의존. 카피로 강조 필요.
- **자동 hot-restart 없음**: allow_external 토글 후 사용자가 앱 재시작 — UX 마찰. v1.x 이월.
- **사내망 audit 미분리**: ring buffer 전역 그루핑 X. Phase 8'.c.5 (v1.x).

## Alternatives considered + rejected

상세는 결정 노트 §2 참조. 핵심:

### 1. LMmaster 자체에 cloudflared launcher 통합

ADR-0055 "외부 통신 0" + ADR-0007 정신 위반. SaaS(Cloudflare) 의존 = thesis 깨짐. 사용자가 별도 도구로 띄우는 건 LMmaster 외부 — OK.

### 2. Origin 필드 완전 삭제

power user의 정확 origin 매칭 시나리오 보존 필요. 라디오 = 의도, origin = 정밀 enforcement — 두 axis 직교. 고급 설정에 보존.

### 3. 자동 게이트웨이 hot-restart

in-flight SSE / 진행 요청 graceful shutdown 복잡. v1.x 이월.

### 4. CIDR 화이트리스트

복잡도 대비 가치 적음. RFC 1918 전체 허용이 90% 시나리오 충족. 정밀 제어는 origin으로.

### 5. 검색 가능한 모델 dropdown

평균 3~10 모델 사용자 시나리오에서 체크박스 인지 부담 더 낮음. 100+ 도달 시 v1.x.

### 6. Connect Mode (PC ↔ PC mesh) 으로 통합

ADR-0058이 "사용자 소유의 *두 LMmaster 인스턴스* 페어링" — 본 시나리오는 "*익명 브라우저*가 LMmaster 호출"이라 패턴 다름. 별 트랙 유지.

## Open follow-ups (v1.x)

- 게이트웨이 hot-restart (allow_external 토글 즉시 반영).
- LAN audit log 분리 (현재 ring buffer 전역).
- LAN URL QR 코드 (동료 폰 셋업 편의).
- API 키 메타 export (CSV/JSON).
- 모델 검색 가능 dropdown (카탈로그 100+ 시).
- ApiKeyEditPanel — 발급 후 alias / network_scope 변경 (현재 회수 후 재발급).

## References

- `crates/key-manager/src/scope.rs::Scope::network_scope` — 데이터 모델.
- `crates/core-gateway/src/auth.rs::require_api_key` — IP enforcement.
- `crates/core-gateway/src/config.rs::GatewayConfig::allow_external` — 바인딩 분기.
- `apps/desktop/src-tauri/src/settings/mod.rs::UserSettings::gateway_allow_external` — persistence.
- `apps/desktop/src/components/keys/ApiKeyIssueModal.tsx` — 라디오 + multi-select.
- `apps/desktop/src/components/keys/ApiKeyRevealStep.tsx` — 신규 분리.
- `apps/desktop/src/components/keys/ApiKeysPanel.tsx` — 카드 뱃지.
- `apps/desktop/src/pages/Settings.tsx` — 사내망 노출 섹션.
- `local-ip-address` 0.6+ — LAN IP 감지.

---

**상태**: Accepted (사용자 명시 승인 — "정식 (A)로 구현 가자"). 결정 노트 §0~§6 완성. Sub-phase 1 진입.
