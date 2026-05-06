# ADR-0058 — User-owned Mesh Connectivity (Connect Mode, Cloud-zero 재정의)

* **상태**: Proposed (2026-05-06 작성). v1.x 안정화 종결 후 v2.0 메이저 분기로 진입 예정 (사용자 결정 T2).
* **선행**:
  - ADR-0013 (Gemini boundary — 외부 통신 0 정책의 원형).
  - ADR-0055 (Network 정책 강화 — `.no_proxy()` 강제 + 폴백 제거).
  - ADR-0010 (Korean-first) — UX/카피 톤 일관.
  - ADR-0014 (Curated Model Registry) — 카탈로그 미러 시 schema 호환.
  - ADR-0046 (Gateway Metrics Middleware) — Connect Mode audit 이벤트가 기존 metrics 파이프 재사용.
* **결정 노트**: `docs/research/phase-20p-connect-mode-decision.md`
* **보강 리서치**: `docs/research/phase-20p-connect-mode-reinforcement.md`

## 컨텍스트

사용자 요청 (2026-05-06): PC1(저사양 노트북) ↔ PC2(RTX 4090 데스크톱) 같은 사용자 소유의 두 LMmaster 인스턴스가 페어링해 (a) 채팅 게이트웨이 라우팅, (b) 카탈로그 read 미러, (c) 모델 설치 위임이 가능한 *Connect Mode*. 본 시나리오는 **localhost-only 바인딩 정책 (외부 통신 0)** 의 명시적 위반으로, thesis 재정의 + 새 보안 모델 정의가 필요하다.

핵심 충돌:
- 기존 정책: "localhost-only 바인딩, no-proxy" (CLAUDE.md §4.2). 프로젝트 정체성의 일부.
- 사용자 가치: 한국 가정/사무실의 *고사양 데스크톱 + 저사양 노트북* 전형 시나리오. 같은 사용자 소유의 두 인스턴스가 협업하지 못하면 4090을 노트북 쪽 채팅에 활용 불가 → LMmaster의 효용이 단일 PC에 갇힘.

ngrok / Cloudflare Tunnel 직접 셋업 우회는 *인증 0* + *EULA 우회* + *audit 부재*로 보안/UX 둘 다 깨진다 (Ollama OLLAMA_HOST=0.0.0.0 반면교사). 따라서 본 ADR이 새 보안 모델을 정의한다.

## 결정

### 1. Thesis 재정의 — "Cloud-zero, user-owned mesh OK"

기존 "외부 통신 0"의 *진의*는 **제3자 클라우드/SaaS 의존 거부**였다. 사용자가 소유한 또 다른 LMmaster 인스턴스와의 통신은 본 정책의 정신을 위반하지 않는다. 본 ADR이 정확한 경계를 명시한다.

| 영역 | v1 정책 (변경 없음) | Connect Mode 활성 시 (v2.0+) |
|---|---|---|
| HuggingFace Hub / Ollama Hub / GitHub Releases | 화이트리스트 도메인만, `.no_proxy()` | 변경 없음 — 호스트 PC만 외부 호출, 게스트는 호스트를 통해 위임 |
| Localhost 게이트웨이 | 127.0.0.1 only | **Connect Mode 활성 + 페어링 후**: 추가 인터페이스 LAN(v1) 또는 iroh QUIC(v2.1) 바인딩 |
| 제3자 클라우드 의존 | 0 | **0 유지** — relay 셀프호스팅 권장, SaaS 0 |

### 2. 네트워크 모델 — 2단계 진입

**v2.0 (Phase 20'.a~e): LAN-only**
- 디스커버리: mDNS (`_lmmaster._tcp.local.` 서비스 타입). TXT 레코드에 `device_id` (blake3 fingerprint) + `protocol_version` + `accent_role`(host/guest).
- 라이브러리: `mdns-sd` 0.18+ (Apache-2/MIT, 순수 Rust, C 의존성 0). astro-dnssd / zeroconf는 Bonjour SDK / avahi-compat 의존이라 portable runtime 정책 위반 → 거부.

**v2.1 (Phase 20'.f): WAN-P2P (옵트인)**
- 라이브러리: `iroh` 0.97+ (n0-computer, Apache-2/MIT). QUIC + 홀펀칭 + relay fallback. 2026-05 시점 production 운영(수십만 디바이스).
- relay 정책: **사용자 수동 토글** — 기본 비활성. 사용자가 자체 relay URL 입력 시에만 WAN 활성화 → cloud-zero 정체성 보존.

### 3. 페어링 (PAKE) + 영구 ID

**페어링 1회**: `magic-wormhole` (Rust, EUPL-1.2/MIT) 사용. SPAKE2 PAKE + 6자리 nameplate + 단어 시퀀스 mnemonic("4-purple-sausage"). UI는 호스트 측 6자리 코드 + 단어 + QR 코드 동시 노출, 게스트는 입력 또는 QR 스캔.

**페어링 후**: PAKE 단발 세션키로 mTLS 키쌍 + `device_id` 영구 교환. **이후 모든 mDNS 디스커버리는 device_id 인증** (Syncthing 모델). PAKE는 첫 페어링 시에만 사용.

### 4. 전송 보안 — mTLS 자체 mini-CA

**선택**: `rustls` 0.23+ mTLS + 자체 mini-CA (camelop/rust-mtls-example 패턴).

페어링 시 양쪽이 self-signed cert 발급 + 서로 client cert를 cross-sign. 이후 mTLS 통신은 `device_id` 기반 mutual auth. 기존 Tauri/axum/rustls 스택 재사용 → IPC integration cost 최소.

`snow` (Noise XX)는 대안으로 검토했으나 별도 framing layer 필요 + 기존 HTTP 스택 재활용 못함 → 거부.

### 5. 권한 모델 — Macaroon-lite caveat + 영구 매니페스트

**역할 (R1~R3 점진 도입)**:
- **R1 — chat-route** (Phase 20'.c): 게스트가 호스트 채팅 게이트웨이 호출만. 가장 안전. 자동 승인 가능.
- **R2 — catalog-read** (Phase 20'.d): 게스트가 호스트 카탈로그/모델 목록 read. 설치 트리거 X. 자동 승인.
- **R3 — model-install-delegate** (Phase 20'.e): 게스트가 호스트에 모델 설치 명령. **매번 호스트 UI 명시 동의 prompt** + macaroon caveat로 *어떤 카탈로그 슬롯까지* 제한 가능.
- **R4 — full mirror** (v2.x 후순위): RAG/워크스페이스/API 키 미러. 보안 위험 큼 → v1 명시 거부.

**저장**: HuJSON `roles.json` (Tailscale ACL 단순화) — 디스크 영구화 + 매번 회수 가능 (Notion share 모델). in-memory only 거부 — UX가 매 세션 입력 부담.

**JWT 거부 이유**: macaroon HMAC 기반은 *발급자=검증자가 동일 LMmaster 인스턴스*라 symmetric 부담 0. JWT 비대칭 장점은 인증서버 분리 시 의미 있는데 본 시나리오는 P2P → 거부.

### 6. UX 정책

- **호스트 UI**: 헤더 우상단 *연결된 기기 수 뱃지* + drawer에 모든 세션 + "지금 모두 끊을게요" 1버튼 킬 스위치 (해요체 한국어).
- **게스트 UI**: 호스트 device alias("거실 PC") 표시, 실제 IP/포트는 상세보기에서만 노출 (Tailscale MagicDNS UX).
- **R3 동의 prompt**: VS Code Live Share 패턴 — *매번 명시 승인*. R1/R2는 자동.
- **연결 진입점**: 설정 → "기기 연결" 별도 탭. 설정 안 하면 기존 단일 PC UX와 100% 동일 (옵트인).

### 7. 카탈로그 동기화 — Pull + ETag (v1)

게스트가 호스트에 manifest를 *필요 시 pull* (push broadcast 거부). ETag로 캐시 검증 → LAN 트래픽 최소. v1.x 시점 카탈로그는 *작고 변경 드물어* CRDT 과잉. v2.x에서 사용자 노트/즐겨찾기 등 mutable 항목 늘면 `automerge` 3.x 도입 검토.

### 8. 감사 로그 + 킬 스위치

기존 `audit-log` infra (R-E.7 cancel_scope에서 검증된 wiring) 재사용. 신규 이벤트:
- `pair-init` / `pair-accept` / `pair-revoke`
- `permission-grant` / `permission-revoke` (role × device_id × timestamp)
- `chat-route` (게스트 → 호스트 추론 위임)
- `catalog-fetch` (R2)
- `model-install-delegate` (R3, 호스트 동의 prompt 결과 포함)

킬 스위치 1버튼 = 모든 device_id 세션 종료 + roles.json 즉시 비움.

## 근거

- **mdns-sd / magic-wormhole / rustls / iroh / snow / automerge — 모두 Apache-2/MIT** → ADR-0010 한국어-우선 정체성과 라이선스 정합. 한국어 docs 미흡은 LMmaster 자체 결정 노트 + 한국어 카피로 흡수.
- **iroh 0.97 production-grade**: 200k 동시 연결, relay public API, 2026-05 활발 갱신. *raw quinn*보다 통째로 채택이 IPC integration cost ↓.
- **Tailnet Lock 부분 채택**: R3 위임 권한만 "인증된 노드만 발급" 패턴 적용 — R1/R2는 과잉.
- **Petals DHT pull-on-demand 흡수**: 카탈로그 미러도 push가 아닌 pull → LAN 트래픽 최소 + 게스트 PC 가용 시점에만 동기화.
- **macaroon caveat 매니페스트 호환**: 기존 카탈로그 매니페스트가 이미 슬롯 ID 보유 → R3 caveat에 slot_id 제한 cost 낮음.

## 거부된 대안

1. **Tailscale 자체 의존**: SaaS 의존 = 정체성 위반. Headscale 셀프호스팅 강제도 사용자 진입 장벽 큼.
2. **ngrok / Cloudflare Tunnel 직접 가이드**: 인증 0 + EULA 우회 + audit 부재. Ollama OLLAMA_HOST=0.0.0.0 반면교사.
3. **rust-libp2p 채택**: DCUtR 홀펀칭 production 검증되지만 본 시나리오엔 과잉 스택. iroh가 LMmaster fit.
4. **webrtc-rs**: sans-IO로 powerful하지만 IPC integration cost 큼. v2.1 iroh 채택으로 충분.
5. **Petals 통째 임베드**: P2P 분산 *추론 자체*는 본 프로젝트 스코프 외. 메타데이터 광고 패턴만 흡수.
6. **R4 full mirror v2.0 진입**: RAG/API 키 미러는 보안 위험 큼 + UX 복잡. R3까지 검증 후 v2.x 검토.
7. **JWT 권한 토큰**: 비대칭 장점이 P2P 시나리오에 무의미. macaroon HMAC가 정합.
8. **WebAuthn / Passkey 페어링**: 브라우저 표준 강결합 → desktop native UX 어색. PAKE+QR이 데스크톱 정합.
9. **Syncthing 통째 임베드**: 파일 단위 동기화가 *구조화된 매니페스트 미러*에 미스매치.
10. **Yjs CRDT**: JS-first → Tauri Rust crate 자연스러움 ↑. v2.x 시점 `automerge` 3.x 우선.
11. **자체 STUN 서버 동봉**: cloud-zero 정체성 위반. iroh-relay 셀프호스팅 옵션이 충분.
12. **Connect Mode 헤더 기본 노출**: v1 안정화 후 진입 + 옵트인 토글이 안전. 설정 → "기기 연결" 탭이 정합.
13. **mDNS without TXT schema_version**: 미래 v2.1+ mixed-version 호환성 불가 → schema_version + 한국어 안내 카피 필수 ("이 기기는 더 새 버전이에요").
14. **인증 없는 0.0.0.0 바인딩 toggle 노출**: 설정에 노출 자체 거부. Connect Mode = 페어링 + mTLS만 허용.

## 결과 / 영향

### 신규 crate (예상)
- `crates/mesh-discovery` — mdns-sd wrapper + TXT schema.
- `crates/mesh-pairing` — magic-wormhole wrapper + device_id 영구화.
- `crates/mesh-transport` — rustls mTLS + 자체 mini-CA + iroh adapter (v2.1 feature flag).
- `crates/mesh-acl` — roles.json HuJSON parser + macaroon caveat.

### IPC 추가
- `mesh.start_advertise()` / `mesh.stop_advertise()` — 호스트 측 mDNS 광고 제어.
- `mesh.start_discovery()` — 게스트 측 LAN 스캔.
- `mesh.create_pairing_code()` — 호스트 6자리 + QR 발급 (5분 시한).
- `mesh.consume_pairing_code(code)` — 게스트 페어링 시도.
- `mesh.list_devices()` / `mesh.revoke_device(device_id)` — 호스트 관리.
- `mesh.set_role(device_id, role)` / `mesh.revoke_role(device_id, role)` — 권한 토글.
- `mesh.kill_all_sessions()` — 1버튼 킬 스위치.

### 백워드 호환
- Connect Mode = **명시적 옵트인**. 기존 단일 PC 사용자에게 영향 0.
- 카탈로그 매니페스트 schema_version 호환 — Phase 20'.d 시점 미러 protocol bump 시 명시.
- 기존 게이트웨이 (127.0.0.1:8788)는 Connect Mode 활성 후에도 *동일 포트* 유지 — 신규 LAN/QUIC 인터페이스만 추가.

### EULA / 보안 감사 영향
- EULA에 Connect Mode 활성 시 *데이터 흐름* 명시 ("페어링한 다른 기기가 채팅 호출/카탈로그 조회 가능").
- 보안 감사 (Phase 7'.b 또는 v2.0 보안 라운드)에서 PAKE 구현 + mTLS mini-CA + macaroon caveat 정확성 검증 필요.
- `.no_proxy()` 정책 (ADR-0055) 유지 — Connect Mode 통신은 *직접 LAN/QUIC* 이라 proxy 무관.

### 라이선스
- mdns-sd (Apache-2/MIT) / magic-wormhole.rs (EUPL-1.2/MIT 듀얼) / rustls (Apache-2/MIT) / iroh (Apache-2/MIT) / snow (Apache-2/MIT) / automerge (MIT) — 모두 LMmaster 라이선스 정체성 정합.

## 테스트 invariant (sub-phase별)

- **20'.a (mDNS 디스커버리)**: TXT schema 호환성, IPv4/IPv6 듀얼, 비활성 시 광고 0.
- **20'.b (PAKE 페어링)**: 6자리 코드 5분 만료, 잘못된 코드 timing-safe 거부, mTLS 키쌍 round-trip.
- **20'.c (R1 chat-route)**: device_id 인증 우회 거부, 권한 회수 즉시 반영, audit 이벤트 정확.
- **20'.d (R2 catalog-read)**: ETag 캐시 invalidate, manifest schema_version 미스매치 한국어 안내.
- **20'.e (R3 model-install-delegate)**: 호스트 동의 prompt 거부 시 게스트 에러, macaroon caveat 슬롯 제한, audit 이벤트 매번 기록.
- **20'.f (WAN/iroh)**: relay URL 미입력 시 WAN 비활성, LAN-only 모드 보존, schema_version 호환.

## 다음 단계

1. **Phase 20' 결정 노트** — 본 ADR과 짝지어 6-section + 8영역 설계 + 9~14 거부된 대안 매트릭스.
2. **sub-phase 분할** — 20'.a~f 6단계 + 각 sub-phase 보강 리서치 spike + DoD.
3. **v1.x 안정화 종료 후 진입** (사용자 결정 T2) — release tag 결정, OCR 카테고리, Workbench batch UI 등 v1.x 작업 종결 후.
