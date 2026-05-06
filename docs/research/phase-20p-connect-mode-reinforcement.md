# Phase 20' — Connect Mode 보강 리서치 (엘리트 사례 종합)

> **목적**: PC1↔PC2 LMmaster 페어링 + 채팅 라우팅 + 카탈로그 미러 + 모델 설치 위임 시 외부 통신 0 + 한국어-우선 + 큐레이션 매니페스트 정체성을 보존하는 라이브러리/패턴 final pick.
> **작성일**: 2026-05-06
> **결정 노트**: `phase-20p-connect-mode-decision.md`
> **ADR**: `docs/adr/0058-user-owned-mesh-connect-mode.md`

---

## 1. 디스커버리 (LAN, v1)

**BEST: `mdns-sd` (keepsimple1)** — 2026-05 v0.18.x, 다운로드 200만+, 25일 전 갱신, 순수 Rust(C 의존성 0), Win/macOS/Linux + IPv4/IPv6, sync/async 무관 (별도 thread + flume channel).

| 후보 | 거부 이유 |
|---|---|
| `astro-dnssd` | Bonjour SDK / avahi-compat 의존 → portable runtime 정책 위반 |
| `zeroconf` | native wrapper 계열, 동일 이유 |

**LMmaster 적용**:
- 서비스 타입: `_lmmaster._tcp.local.`
- TXT 레코드: `device_id`(blake3 fingerprint) + `protocol_version` + `accent_role`(host/guest)
- *외부 통신 0*에 위배 없는 LAN broadcast만으로 P0 디스커버리 완결.

**Tailscale UX 흡수 포인트**:
- MagicDNS는 사용자에게 IP/포트를 절대 노출시키지 않음 → LMmaster도 사용자 화면엔 device alias("거실 PC")만, 실제 IP/포트는 상세보기에서만.

**Syncthing 패턴 흡수**:
- *device_id = 공개키 fingerprint* — 인증 단위와 디스커버리 단위를 일치시킴. LMmaster 동일 패턴 채택.

---

## 2. 페어링 (PAKE + QR, v1)

**BEST: `magic-wormhole` (Rust)** — 2026-02-03 갱신, SPAKE2 PAKE 표준, 6자리 nameplate + word-list mnemonic("4-purple-sausage"). MIT/Apache-2 듀얼.

- Rendezvous 서버 self-host 가능 (`magic-wormhole-mailbox-server`).
- Croc은 wormhole 호환 + 친화적이지만 Go 단일 바이너리 위주라 Rust 임베드는 wormhole.rs가 우위.

**LMmaster 적용**:
1. 호스트 측 "기기 추가" 버튼 → 6자리 코드 + 단어 시퀀스 + QR (코드 그대로 인코딩).
2. 게스트는 코드 입력 또는 QR 스캔.
3. SPAKE2로 단발 세션키 도출.
4. 그 키로 mTLS 키쌍 + device_id 영구 교환.
5. **이후 mDNS 디스커버리는 device_id로만 인증** (Syncthing 모델). 첫 페어링 시에만 PAKE.

**v2.1 Transit relay 자체호스팅 옵션**: `magic-wormhole-transit-relay` — *기본 LM Studio 관리자가 자체 relay 운영 가능*하게 함 → 외부 통신 0 정체성과 정합. v1엔 LAN-only이므로 relay 자체가 필요 없음.

---

## 3. 전송 보안 + P2P 전송 (v1=LAN, v2.1=WAN)

### v1 LAN: rustls 1.x mTLS + (선택) `snow` Noise XX

- rustls는 self-signed 단독 trust anchor 한계 있음 → 페어링 시 *자체 mini-CA*를 만들고 양쪽이 서로 client cert를 cross-sign (camelop/rust-mtls-example 패턴).
- Noise XX는 mTLS 대안 — `snow` (mcginty) crate가 표준, WhatsApp/WireGuard가 채용한 Noise framework.
- **LMmaster 결정**: mTLS — 기존 axum/rustls 스택 재사용 → IPC integration cost 최소.

### v2.1 WAN: `iroh` 0.97 (n0-computer)

- 2026-05 시점 *수십만 디바이스에서 200k 동시 연결 production* 운영.
- QUIC(자체 구현 noq) + 홀펀칭 + relay fallback 통합.
- 0.97에서 RelayService public + axum 통합 가능 → **LM Studio가 자체 relay 운영하면 cloud-zero 정체성과 정합**.
- iroh-net을 직접 임포트해 1.0 직전까지 wire-stable 직전 단계.

**LMmaster 적용**: v2.1 분기 시 *raw quinn*보다 iroh를 통째로 채택. 사용자가 외부 노출을 원하지 않으면 LAN-only 모드 유지(iroh도 LAN discovery 지원).

**기각**:
- `rust-libp2p` — DCUtR 홀펀칭 production 측정치 (2.5M 성공/6.25M 시도) 있지만 *LMmaster 시나리오엔 과잉 스택*.
- `tailscale-rs` — "production 금지" 자체 경고 있음.
- `webrtc-rs` — sans-IO로 powerful하지만 IPC integration cost가 iroh보다 큼.

---

## 4. 권한 모델 (R1/R2/R3)

**BEST: Tailscale-style HuJSON ACL 단순화 + Macaroon-lite caveat**.

- Tailscale ACL: user/group/tag 3축 + JSON capability — Headscale 0.23+이 동일 구조를 self-host 보존.
- Capability-based(Cap'n Proto, Fuchsia)는 본 시나리오엔 과잉.

**LMmaster 적용**:
- `roles: { chat-route: r1, catalog-read: r2, model-install-delegate: r3 }` 매니페스트 토큰.
- 토큰에 caveat (time-bound expiry, scope=catalog-only, max-bytes) attach 가능 → 위임 시 권한 축소만 가능 (macaroon의 핵심 invariant).
- 게스트가 다시 다른 게스트에 *권한 확장 없이* 재위임 가능.

**JWT 채택 안 한 이유**:
- macaroon은 HMAC 기반 + 발급자/검증자가 동일 LMmaster 인스턴스라 *symmetric 부담이 0*.
- JWT 비대칭 장점은 인증서버 분리 시점에 의미 있는데 본 시나리오는 P2P라 발급=검증이 한 노드.
- WebAuthn/Passkey는 *브라우저 표준* 강결합이라 desktop native에서 UX 어색.

---

## 5. UX 베스트 프랙티스 흡수

### VS Code Live Share
- 호스트가 게스트 join을 *명시 승인* + 언제든 "Remove participant" 1클릭 + 터미널은 read-only 기본 → write 토글은 개별 승격.
- **LMmaster 적용**: R1/R2 자동 승인 + R3(모델 설치) 매번 명시 승인, 호스트 헤더에 *현재 연결된 게스트 + 라이브 활동* 항상 노출. 킬 스위치 1버튼.

### LM Studio Server (반면교사)
- 인증 미적용 시 노출 위험을 자체 docs가 경고 → LMmaster는 *인증 없는 0.0.0.0 바인딩 자체를 차단*하는 것이 정합.
- Ollama OLLAMA_HOST=0.0.0.0 인증 없음 노출은 보안 사고 유발.

### Petals 2.0 DHT
- NAT/relay peer는 직결 가능 peer에 키 위탁 + swarm monitor에 server 메타(LoRA 등)를 DHT에 publish → ping 없이 일람.
- **LMmaster 흡수**: 카탈로그 미러도 push가 아닌 *pull-on-demand*로 시작, 메타데이터(모델 이름·VRAM·라이선스)만 작은 manifest로 광고.

### Tailnet Lock
- 새 노드는 *기존 신뢰 노드 서명*이 있어야 join → LMmaster에선 R3(설치 위임) 권한만 동일한 "인증된 노드만 위임 발급" 패턴 채택. R1/R2엔 과잉.

---

## 6. 동기화 / 카탈로그 미러

**BEST: Automerge 3.x (Rust core)** — JSON 기반, 2026 시점 production, 충돌 자동 머지. Yjs는 JS-first이고 LMmaster는 Tauri라 native crate가 자연스러움.

*카탈로그 매니페스트는 작고 변경이 드물어 CRDT가 과잉*일 수 있으므로 v1에선 단순 *manifest version vector + last-writer-wins per slot*로 시작 → v2.x에서 사용자 노트/즐겨찾기 등 mutable 항목 늘면 Automerge 도입.

**기각**:
- Syncthing 통째 임베드 — 파일 단위 동기화가 본 시나리오의 *구조화된 매니페스트 미러*에 미스매치.
- Git-as-transport — 사용자 화면에 history 노출이 어색.

---

## 7. 감사 로그 / 킬 스위치

**Tailscale audit log**가 가르치는 것: action / actor / target / time + old vs new 값.

**LMmaster 적용**: 기존 `audit-log` infra(R-E.7 cancel_scope에서 검증된 wiring)에 connect-mode 이벤트 추가 — `pair-init`, `pair-accept`, `permission-grant`, `permission-revoke`, `chat-route`, `catalog-fetch`, `model-install-delegate`. 모든 이벤트에 device_id + role + timestamp.

**킬 스위치 UI**: 헤더 우상단 *연결된 기기 수 뱃지* 클릭 → drawer에 모든 세션 + "전체 끊기" CTA(해요체: "지금 모두 끊을게요"). prefers-reduced-motion 토큰 준수.

---

## 8. 권장 Rust crate final pick (Apache-2/MIT)

| 영역 | crate | 라이선스 | 안정성 |
|---|---|---|---|
| LAN 디스커버리 | `mdns-sd` 0.18+ | Apache-2/MIT | 200만+ DL, 활성 유지 |
| 페어링 | `magic-wormhole` (Rust) | EUPL-1.2/MIT 듀얼 | 2026-02 갱신, mature |
| Noise (선택) | `snow` 0.10+ | Apache-2/MIT | mature, 광범위 채택 |
| TLS | `rustls` 0.23+ | Apache-2/MIT | 사실상 Rust 표준 |
| QUIC v2.1 | `iroh` 0.97 (or `quinn` 직접) | Apache-2/MIT | iroh production-grade |
| CRDT v2.x | `automerge` 3.x | MIT | 2.0+ production ready |

iroh / magic-wormhole.rs / snow / mdns-sd / quinn은 모두 *2026-현재 활발히 유지 + 정식 Apache-2 또는 MIT* 으로 기존 LMmaster 라이선스 정체성과 정합. 한국어 docs는 모두 미흡 — 이 부분은 LMmaster가 자체 결정 노트 + 한국어 카피로 흡수해야 할 영역.

---

## 9. 결정 포인트 (큐레이터/사용자 선택 결과)

| # | 결정 포인트 | 권장 (채택) | 거부 / 후순위 |
|---|---|---|---|
| 1 | v2.1 relay 셀프호스팅 모드 | **사용자 수동 토글** (cloud-zero 보존) | 자체 relay 동봉 거부 |
| 2 | 첫 페어링 후 transport | **mTLS** (기존 axum/rustls 재사용) | Noise XX (별도 framing 필요) |
| 3 | 권한 모델 표현 | **HuJSON 영구화 + 매번 회수 가능** | in-memory only (UX 부담) |
| 4 | 카탈로그 sync v1 전략 | **pull + ETag** (외부 통신 0 정합) | push-on-change (LAN 트래픽↑) |
| 5 | R3 위임 안전망 | **caveat 도입** (slot_id 제한) | 통째 on/off (큐레이션 cost 낮음에도 흘림) |
| 6 | mDNS schema_version 호환 | **schema_version + 한국어 안내 카피** | 무버전 (mixed-version 불가) |
| 7 | Connect Mode 진입 토글 | **설정 → "기기 연결" 별도 탭** (옵트인) | 헤더 기본 노출 (v1 안정화 위협) |

---

## 10. 출처 (Sources)

- [iroh (n0-computer)](https://github.com/n0-computer/iroh)
- [iroh 0.97.0 release](https://www.iroh.computer/blog/iroh-0-97-0-custom-transports-and-noq)
- [iroh 0.96.0 release](https://www.iroh.computer/blog/iroh-0-96-0-the-quic-multipaths-to-1-0)
- [iroh-relay crate](https://crates.io/crates/iroh-relay)
- [magic-wormhole.rs](https://github.com/magic-wormhole/magic-wormhole.rs)
- [magic-wormhole crate](https://crates.io/crates/magic-wormhole)
- [magic-wormhole transit relay](https://github.com/magic-wormhole/magic-wormhole-transit-relay)
- [mdns-sd (keepsimple1)](https://github.com/keepsimple1/mdns-sd)
- [astro-dnssd (AstroHQ)](https://github.com/AstroHQ/astro-dnssd)
- [snow Noise Protocol Rust](https://github.com/mcginty/snow)
- [quinn Rust QUIC](https://github.com/quinn-rs/quinn)
- [rust-libp2p hole punching](https://docs.rs/libp2p/latest/libp2p/tutorials/hole_punching/index.html)
- [webrtc-rs RTC](https://github.com/webrtc-rs/rtc)
- [Tailscale ACL syntax](https://tailscale.com/docs/reference/syntax/policy-file)
- [Tailscale audit logging](https://tailscale.com/docs/features/logging/audit-logging)
- [Tailnet Lock white paper](https://tailscale.com/docs/concepts/tailnet-lock-whitepaper)
- [Headscale ACLs](https://headscale.net/stable/ref/acls/)
- [Syncthing device IDs](https://docs.syncthing.net/dev/device-ids.html)
- [Syncthing global discovery v3](https://docs.syncthing.net/specs/globaldisco-v3.html)
- [VS Code Live Share security](https://learn.microsoft.com/en-us/visualstudio/liveshare/reference/security)
- [LM Studio authentication](https://lmstudio.ai/docs/developer/core/authentication)
- [Petals (BigScience)](https://github.com/bigscience-workshop/petals)
- [Macaroons (Wikipedia)](https://en.wikipedia.org/wiki/Macaroons_(computer_science))
- [Automerge 2.0](https://automerge.org/blog/automerge-2/)
- [rust-mtls-example](https://github.com/camelop/rust-mtls-example)
