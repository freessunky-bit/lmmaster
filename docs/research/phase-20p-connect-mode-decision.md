# Phase 20' — Connect Mode 결정 노트 (User-owned Mesh)

> **작성일**: 2026-05-06
> **트리거**: 사용자 요청 — PC1↔PC2 LMmaster 페어링 + 채팅 라우팅 + 카탈로그 미러 + 모델 설치 위임 ("커넥트 모드")
> **선행 문서**: `docs/adr/0058-user-owned-mesh-connect-mode.md`, `docs/research/phase-20p-connect-mode-reinforcement.md`
> **진입 시점**: v1.x 안정화 종결 후 v2.0 메이저 분기 (사용자 결정 T2)

---

## 1. 결정 요약

- **A1**: Thesis 재정의 — "외부 통신 0 → Cloud-zero, user-owned mesh OK". 제3자 클라우드/SaaS 의존 0 유지, 사용자 소유 인스턴스끼리 통신 허용. ADR-0058 신설.
- **A2**: 네트워크 2단계 — v2.0 LAN-only (mdns-sd) → v2.1 WAN-P2P (iroh, 사용자 수동 relay 토글).
- **A3**: 페어링 — `magic-wormhole` SPAKE2 PAKE + 6자리 + QR. 첫 페어링 시에만 사용, 이후 device_id 영구.
- **A4**: 전송 — `rustls` mTLS + 자체 mini-CA (cross-sign). 기존 axum/rustls 스택 재사용.
- **A5**: 권한 — R1(chat-route) → R2(catalog-read) → R3(model-install-delegate) 3단계. R4(full mirror) v1 거부. HuJSON `roles.json` 영구화 + macaroon caveat.
- **A6**: UX — 옵트인 (설정 → "기기 연결" 별도 탭), 호스트 헤더에 연결 기기 + 라이브 활동 + 1버튼 킬 스위치.
- **A7**: 동기화 — 카탈로그 pull + ETag. v2.x에 automerge CRDT 검토.
- **A8**: 감사 로그 — 기존 `audit-log` infra 재사용, 7개 신규 이벤트.

---

## 2. 채택안 — 8영역 설계

### 2.1 디스커버리 (mDNS, v2.0)

| 항목 | 값 |
|---|---|
| 라이브러리 | `mdns-sd` 0.18+ (Apache-2/MIT, 순수 Rust) |
| 서비스 타입 | `_lmmaster._tcp.local.` |
| TXT 레코드 | `device_id` (blake3) + `protocol_version` + `accent_role`(host/guest) + `display_name`("거실 PC") |
| 광고 정책 | Connect Mode 비활성 시 광고 0 (옵트인) |
| 듀얼 스택 | IPv4 + IPv6 |

UI: 게스트 측 LAN 스캔 시 device alias만 노출 (Tailscale MagicDNS 패턴), IP/포트는 상세보기에서.

### 2.2 페어링 (PAKE + QR, v2.0)

1. 호스트 → "기기 추가" 버튼 → 6자리 코드 + 단어 mnemonic("4-purple-sausage") + QR 코드 동시 노출. 5분 시한.
2. 게스트 → 코드 입력 또는 QR 스캔.
3. SPAKE2로 단발 세션키 도출.
4. 그 키로 양쪽이 self-signed cert 발급 + cross-sign → mTLS 키쌍 + device_id 영구 저장.
5. 이후 mDNS 디스커버리는 device_id 인증.

라이브러리: `magic-wormhole` (Rust, EUPL-1.2/MIT). Rendezvous 서버는 v2.0 LAN-only라 불필요. v2.1 시점 셀프호스팅 옵션.

### 2.3 전송 보안 (mTLS, v2.0) + iroh QUIC (v2.1)

**v2.0 LAN**: rustls 0.23+ mTLS + 자체 mini-CA (camelop/rust-mtls-example 패턴). 양쪽이 client cert를 cross-sign.

**v2.1 WAN**: iroh 0.97+ — QUIC + 홀펀칭 + relay fallback. 사용자가 자체 relay URL 입력 시에만 활성. 기본 비활성 → cloud-zero 정체성 보존.

### 2.4 인증 (device_id 기반)

- `device_id` = blake3(공개키 fingerprint). Syncthing 모델.
- 페어링 후 영구 저장 (`workspace/connect/devices.json`, SQLCipher 암호화).
- mTLS handshake 시 device_id로 known peers 매칭. 미인증 디바이스 거부.

### 2.5 권한 (R1~R3 + Macaroon caveat)

| 역할 | 범위 | 승인 정책 |
|---|---|---|
| **R1 chat-route** (20'.c) | 게스트 → 호스트 채팅 게이트웨이 호출 | 자동 |
| **R2 catalog-read** (20'.d) | 게스트 → 호스트 카탈로그/모델 목록 read. 설치 트리거 X | 자동 |
| **R3 model-install-delegate** (20'.e) | 게스트 → 호스트 모델 설치 명령 | **매번 호스트 UI 명시 동의 prompt** + macaroon caveat (slot_id 제한) |
| **R4 full-mirror** | RAG/워크스페이스/API 키 미러 | **v1 거부** — v2.x 검토 |

Macaroon caveat 종류:
- `slot_id = ["nemotron-3-nano-4b", "exaone-3.5-7.8b-instruct"]` — 특정 카탈로그 슬롯 ID 제한
- `expires_at = "2026-06-06T00:00:00Z"` — 시한
- `max_bytes = 5_000_000_000` — 다운로드 한도

저장: HuJSON `roles.json` 디스크 영구화. 매번 회수 가능 (Notion share 모델).

### 2.6 UX

**진입점**: 설정 → "기기 연결" 별도 탭. 옵트인 → 설정 안 하면 기존 단일 PC UX와 100% 동일.

**호스트 화면**:
- 헤더 우상단 *연결 뱃지* — 연결된 게스트 수 표시.
- 클릭 → drawer에 모든 세션 (device alias + role + 라이브 활동 + 마지막 호출 시각).
- 1버튼 킬 스위치: "지금 모두 끊을게요" 해요체.
- R3 동의 prompt: VS Code Live Share 패턴 (모달 — 게스트 alias + 슬롯 ID + "허용/거부").

**게스트 화면**:
- 호스트 alias 표시. 실제 IP/포트는 상세보기에서만.
- 권한 부여 상태 chip ("채팅 OK / 카탈로그 OK / 설치 위임 — 호스트 승인 필요").

**a11y / 톤**: CLAUDE.md §4.1 한국어 톤 유지. focus-visible ring 토큰 (`--primary-a-3`). prefers-reduced-motion 토큰. 한국어 해요체.

### 2.7 동기화 (카탈로그 미러)

**v2.0**: pull-on-demand + ETag. 게스트가 호스트에 manifest 요청 → 변경 없으면 304 Not Modified. 호스트가 manifest 변경 시 broadcast 거부 (LAN 트래픽 최소).

**v2.x**: Automerge 3.x 검토 — 사용자 노트 / 즐겨찾기 / 카탈로그 큐레이션 메모 등 mutable 항목 늘면.

### 2.8 감사 로그 + 킬 스위치

기존 `audit-log` infra (R-E.7 cancel_scope에서 검증된 wiring) 재사용. 신규 이벤트 7종:
- `pair-init` / `pair-accept` / `pair-revoke`
- `permission-grant` / `permission-revoke` (role × device_id × timestamp)
- `chat-route` / `catalog-fetch` / `model-install-delegate`

킬 스위치 1버튼 = 모든 device_id 세션 종료 + roles.json 즉시 비움 (audit `permission-revoke` × N + `pair-revoke` × N 일괄 기록).

---

## 3. 기각안 + 이유 (negative space — 다음 세션 보호)

| 기각안 | 거부 이유 |
|---|---|
| **Tailscale 자체 의존** | SaaS 의존 = 정체성 위반. Headscale 셀프호스팅 강제도 사용자 진입 장벽 큼. |
| **ngrok / Cloudflare Tunnel 가이드** | 인증 0 + EULA 우회 + audit 부재. Ollama OLLAMA_HOST=0.0.0.0 반면교사. |
| **rust-libp2p** | DCUtR production 검증되지만 본 시나리오엔 과잉 스택. iroh가 fit. |
| **webrtc-rs** | sans-IO로 powerful하지만 IPC integration cost 큼. v2.1 iroh 채택으로 충분. |
| **Petals 통째 임베드** | P2P 분산 추론 자체는 본 프로젝트 스코프 외. 메타 광고 패턴만 흡수. |
| **R4 full mirror v2.0 진입** | RAG/API 키 미러 보안 위험 + UX 복잡. R3까지 검증 후 v2.x 검토. |
| **JWT 권한 토큰** | 비대칭 장점이 P2P에 무의미. macaroon HMAC가 정합. |
| **WebAuthn / Passkey** | 브라우저 표준 강결합 → desktop native UX 어색. PAKE+QR 정합. |
| **Syncthing 통째 임베드** | 파일 단위 동기화가 *구조화된 매니페스트 미러*에 미스매치. |
| **Yjs CRDT** | JS-first → Tauri Rust crate (`automerge`)가 자연스러움 ↑. |
| **자체 STUN/relay 서버 동봉** | cloud-zero 정체성 위반. iroh-relay 셀프호스팅 옵션이 충분. |
| **Connect Mode 헤더 기본 노출** | v1 안정화 위협. 설정 → "기기 연결" 옵트인 탭이 정합. |
| **mDNS without TXT schema_version** | 미래 mixed-version 호환성 불가 → schema_version 필수. |
| **인증 없는 0.0.0.0 바인딩 toggle** | 노출 자체 거부. Connect Mode = 페어링 + mTLS만 허용. |
| **astro-dnssd / zeroconf** | Bonjour SDK / avahi-compat 의존 → portable runtime 정책 위반. |
| **Noise XX 단독 (mTLS 거부)** | 별도 framing layer 필요 + 기존 HTTP 스택 재활용 못함. |
| **CRDT v2.0 진입 (automerge 즉시)** | 카탈로그 매니페스트는 작고 변경 드물어 CRDT 과잉. v1은 version vector + last-writer-wins. |
| **첫 페어링 후 PAKE 재사용** | Syncthing 모델 (device_id 영구) 채택 → PAKE는 1회만. |

---

## 4. 미정 / 후순위 이월

- **Phase 20'.b 페어링 UX 디테일** — 6자리 코드 + 단어 mnemonic 표시 vs 단어만 표시 (사용자 테스트 필요).
- **R3 동의 prompt 시점** — 게스트 요청 시 즉시 prompt vs 배치 (예: 5건 모이면 일괄 승인). v1 즉시.
- **Tauri capabilities 갱신** — `mesh.*` IPC 그룹 신설 + capabilities/main.json 등록. ACL drift 방지.
- **EULA 갱신** — Connect Mode 활성 시 데이터 흐름 명시. 한국어/영어 동시.
- **Tauri capability 정책** — Connect Mode 비활성 시 mesh.* IPC 호출 자체 거부 (UI 레벨 + IPC 레벨 dual gate).
- **카탈로그 schema_version 호환성** — Phase 20'.d 시점 미러 protocol bump 시 명시.
- **v2.1 iroh relay URL 검증** — 사용자 입력 URL이 iroh-relay 호환인지 health check.
- **device_id 충돌** — blake3 fingerprint 충돌 확률 ~0이지만, UI에서 동일 alias 다수 시 disambiguation.
- **R3 동의 prompt 우회 공격 벡터** — 게스트가 일부러 빠르게 연속 요청 시 DoS. rate limit 정책.
- **모바일 시나리오** — Tauri Mobile 시점 Connect Mode 모바일 클라이언트는 v2.x.

---

## 5. 테스트 invariant (sub-phase별)

### 20'.a (mDNS 디스커버리)
- TXT schema 호환성 — schema_version 0 ↔ 1 mixed-version에서 한국어 안내 카피.
- IPv4/IPv6 듀얼 스택 동시 광고.
- Connect Mode 비활성 시 광고 0 (네트워크 패킷 캡처로 검증).
- 동일 LAN 다중 LMmaster 인스턴스 동시 디스커버리.

### 20'.b (PAKE 페어링)
- 6자리 코드 5분 만료 (정확한 timer).
- 잘못된 코드 timing-safe 거부 (timing oracle 방어).
- mTLS 키쌍 round-trip + cross-sign 검증.
- QR 코드 round-trip (인코딩/디코딩 일치).
- device_id 영구 저장 후 재시작 시 복원.

### 20'.c (R1 chat-route)
- device_id 인증 우회 거부 (위조 cert 거부).
- 권한 회수 즉시 반영 (회수 후 1ms 내 거부).
- audit 이벤트 정확 (timestamp + device_id + action).
- cancel cascade — 게스트 disconnect 시 호스트 추론 cancel.
- chat 스트림 graceful early disconnect (R-E.6 패턴 재사용).

### 20'.d (R2 catalog-read)
- ETag 캐시 invalidate (manifest 변경 시 304 → 200 전환).
- manifest schema_version 미스매치 한국어 안내.
- 권한 없이 read 시도 거부.

### 20'.e (R3 model-install-delegate)
- 호스트 동의 prompt 거부 시 게스트 한국어 에러 toast.
- macaroon caveat slot_id 제한 (whitelist 외 거부).
- audit 이벤트 매번 기록 (prompt 결과 포함).
- 동시 다중 install 요청 직렬화.

### 20'.f (WAN/iroh)
- relay URL 미입력 시 WAN 비활성 (LAN-only 보존).
- iroh discovery + LAN 동시 활성 시 충돌 없음.
- protocol_version 호환 — v2.0 노드 ↔ v2.1 노드 한국어 안내.

---

## 6. 다음 페이즈 인계 — sub-phase 분할

### 진입 조건
- v1.x 안정화 종료 (release tag + OCR 카테고리 + Workbench batch UI 등).
- 사용자 결정 T2 ("v1.x 마치고 v2.0 진입") 재확인.
- ADR-0058 사용자 명시 승인.
- v2.0 보안 감사 (PAKE / mTLS / macaroon caveat 정확성) 후보 페이즈 (Phase 7'.b 보안 라운드 후속).

### sub-phase 6단계

| Phase | 제목 | 의존성 | DoD |
|---|---|---|---|
| **20'.a** | LAN 디스커버리 (mDNS) | ADR-0058 | `crates/mesh-discovery` + IPC + TXT schema + 4 invariant |
| **20'.b** | PAKE 페어링 (magic-wormhole + mTLS mini-CA) | 20'.a | `crates/mesh-pairing` + `crates/mesh-transport` + UI 6자리/QR + 5 invariant |
| **20'.c** | R1 chat-route | 20'.b | `crates/mesh-router` + 채팅 게이트웨이 라우팅 + cancel cascade + 5 invariant |
| **20'.d** | R2 catalog-read | 20'.c | 카탈로그 pull + ETag + manifest version vector + 3 invariant |
| **20'.e** | R3 model-install-delegate | 20'.d | macaroon caveat + 호스트 동의 prompt + audit 이벤트 + 4 invariant |
| **20'.f** | v2.1 WAN-P2P (iroh) | 20'.e | iroh adapter + relay URL 토글 + protocol_version 호환 + 3 invariant |

### 위험 매트릭스

| 위험 | 영향 | 완화 |
|---|---|---|
| PAKE 구현 버그 (timing oracle 등) | MITM 가능 | `magic-wormhole` 라이브러리 신뢰 + 보안 감사 + timing-safe 비교 강제 |
| mTLS mini-CA 실수 | 인증 우회 | camelop/rust-mtls-example 패턴 정확 따름 + 통합 테스트 |
| macaroon caveat 누수 | 권한 escalation | caveat parser 단위 테스트 + audit log로 사후 추적 |
| mDNS 정보 노출 | 같은 LAN에 device_id 노출 | TXT 레코드 최소화 + 옵트인 |
| 사용자 실수 R3 자동 승인 | 모델 설치 위임 폭주 | 매번 명시 prompt + 일괄 승인 거부 |
| iroh relay 외부 의존 | cloud-zero 정체성 위반 위험 | 사용자 수동 토글 + LAN-only 기본 |
| EULA 미갱신 | 약관 부재 → 법적 리스크 | Phase 20'.b 진입 시 EULA 갱신 의무 |
| 보안 감사 미흡 | 페어링 / 인증 / 권한 결함 | v2.0 ship 전 외부 보안 감사 (Phase 7'.b 후속) |

### 다음 standby (v1.x 마무리 후)
- Phase 20'.a 진입 — `crates/mesh-discovery` + mdns-sd 통합 + TXT schema 정의.
- 보강 리서치 spike — 각 sub-phase별 추가 리서치 (예: 20'.b PAKE UI 디테일, 20'.f iroh integration 정확한 API).

---

## 7. 검증 체크리스트 (각 sub-phase 종료 시)

CLAUDE.md §7 DoD 따름:

```powershell
.\.claude\scripts\verify.ps1
```

**구현**:
- 결정 노트 6-section 완전 (특히 §3 기각안+이유).
- 신규 모듈에 §5 테스트 invariant 적용.
- 한국어 카피 §4.1 톤 + i18n ko/en 동시.
- UI 변경 시 §4.3 a11y/포커스/키보드.

**검증**:
- `cargo fmt --all -- --check` ✅
- `cargo clippy --workspace --all-targets -- -D warnings` ✅
- `cargo test --workspace` ✅
- `pnpm exec tsc -b` ✅
- `pnpm exec vitest run` ✅ (UI 변경 시)

**문서**:
- `docs/RESUME.md` 갱신.
- 결정 노트 + ADR (필요 시).

**보안**:
- audit 이벤트 wiring 검증 (R-E.7 패턴).
- IPC capabilities 등록 검증 (ACL drift check).
- TLS 키쌍 / macaroon caveat / device_id 검증 단위 테스트.

---

## 출처 (보강 리서치 노트 §10 참조)

`docs/research/phase-20p-connect-mode-reinforcement.md` §10에 25개 출처 링크 보존 — iroh / magic-wormhole.rs / mdns-sd / rustls / snow / quinn / Tailscale / Headscale / Syncthing / VS Code Live Share / LM Studio / Petals / Macaroon paper / Automerge / rust-mtls-example.
