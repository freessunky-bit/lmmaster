# Phase 13'.g.3 — 매니페스트 서명 확장 (catalog 외) 결정 노트

> **작성일**: 2026-05-06
> **트리거**: 사용자 요청 — "catalog 외 manifest signature 확장 진행".
> **선행**: ADR-0047 (catalog minisign Ed25519). Phase 13'.g.2.a~d.
> **스코프**: CI 측 자동 .minisig 생성을 catalog 외 매니페스트로 확장. **runtime verify 통합은 v1.x 후속**으로 명시 분리.

---

## 1. 결정 요약

- **A1**: `sign-catalog.yml` 확장 — paths trigger + sign loop이 `catalog.json` + `ollama.json` + `lm-studio.json` 모두 처리. `.minisig` 자동 생성·commit·push.
- **A2**: workflow 이름 `Sign catalog` → `Sign manifests` 일반화.
- **A3**: registry-fetcher는 *이미 generic 설계* (`signature_url_for(manifest_id, tier)` / `fetch_signature_text` / `mark_signature_verified`) — 호출 측만 추가하면 됨. 그러나 **본 sub-phase에서는 verify 통합 보류**. 이유: 통합 테스트 + fail-fast 정책 + UX 정의 필요. v1.x 별개 sub-phase로 분리.
- **A4**: ADR-0047은 *catalog 단일* 명시 → 확장 시 ADR 갱신 필요하나 **본 sub-phase는 결정 노트만 + ADR-0047 §결과 영향에 후속 명시**. 정식 ADR 갱신은 verify 통합 시점에.

---

## 2. 채택안

### A1. sign-catalog.yml 확장

**변경 전**:
- `paths`: `manifests/apps/catalog.json`만 트리거.
- sign step: `rsign sign` 단일 파일.
- commit step: `catalog.json.minisig` 1개만.

**변경 후**:
- `paths`: `catalog.json` + `ollama.json` + `lm-studio.json` 3종 트리거.
- sign step: 3개 파일 loop (단일 password 입력으로 stdin 재사용).
- verify step: 3개 파일 loop self-check (pubkey 설정 시).
- commit step: 변경된 .minisig만 stage + single commit + push.

**Idempotency**: 매니페스트 변경 없으면 .minisig도 byte-identical → commit skip.

### A2. workflow 이름 일반화

`name: Sign catalog` → `name: Sign manifests`. GitHub Actions UI에 더 명확.

### A3. Runtime verify 통합 — v1.x 분리

registry-fetcher는 이미 **generic**:
- `RegistryFetcher::signature_url_for(manifest_id, tier)` — manifest_id 기반.
- `fetch_signature_text(url, timeout)` — generic .minisig fetch.
- `SignatureVerifier::verify(body, sig)` — 매니페스트 종류 무관.
- `mark_signature_verified(source, manifest_id)` — generic 마킹.

따라서 *호출 측*에 추가만 하면 verify 동작. 그러나 다음 이유로 **본 sub-phase 분리**:
1. **통합 테스트 부담** — desktop의 `verify_catalog_signature` 패턴을 ollama / lm-studio용 `verify_app_manifest_signature`로 일반화 + 6+ 회귀 가드.
2. **fail-fast 정책** — verify 실패 시 (a) bundled fallback / (b) 한국어 에러 toast / (c) Diagnostics 빨간 카드 어디로 라우팅할지 결정 필요.
3. **secret 미설정 graceful** — 현재 catalog 검증은 `LMMASTER_CATALOG_PUBKEY` 미설정 시 verify 비활성. ollama/lm-studio도 같은 정책 vs 별개 secret 정책 결정 필요.
4. **UX 카피** — "Ollama 매니페스트 검증 실패" 한국어 카피 + Settings 토글.

→ **v1.x 별개 sub-phase 13'.g.4** (sign + verify 통합)로 분리.

### A4. ADR 갱신 정책

ADR-0047은 *catalog 단일* 명시. 본 sub-phase는 *CI 측 .minisig 생성*만 확장이라 ADR-0047 §결과/영향에 1줄 추가 ("v1.x phase 13'.g.3에서 ollama.json / lm-studio.json까지 .minisig 자동 생성 확장. verify 통합은 13'.g.4 후속.")로 처리. 정식 ADR-0047 v2 또는 ADR-0059는 13'.g.4 시점에.

---

## 3. 기각안 + 이유

| 기각안 | 거부 이유 |
|---|---|
| **본 sub-phase에서 runtime verify 통합도** | 통합 테스트 + fail-fast UX 정의 부담 큼. v0.0.1 ship 직전 위험 추가 거부. v1.x 분리 |
| **3개 매니페스트 별개 secret keypair** | catalog와 동일 keypair 재사용이 단순 + 정합. 키 회전 시 한 번에 처리 |
| **변경된 매니페스트만 sign loop** | 단순화 위해 3종 모두 매번 sign — idempotent라 byte-identical이면 commit skip. 부수 효과 0 |
| **ADR-0047 v2 즉시 발행** | 본 sub-phase 변경 면적 작음. v1.x 13'.g.4에 verify 통합과 함께 정식 갱신 |
| **manifests/apps 외 (예: snapshot/models/*.json)도 서명** | 카탈로그 1개로 이미 모델 매니페스트 무결성 보장. 개별 모델 .minisig는 GB 단위 .minisig 폭증 + ROI 낮음 |
| **`paths-ignore`로 .minisig 변경은 자기 자신 제외** | 이미 commit message `[skip-sign]` 으로 차단 (line 25 `if !contains(...)`). 추가 paths-ignore 불필요 |

---

## 4. 미정 / 후순위 이월

- **Phase 13'.g.4** — runtime verify 통합 (registry-fetcher 호출 측 + desktop verify_app_manifest_signature + UI 토글). v1.x.
- **secret 정책 일원화 vs 분리** — 현재는 단일 keypair 재사용. 향후 매니페스트별 권한 분리 시 별개 keypair 검토.
- **EULA 갱신** — 매니페스트 서명 정책 명시 ("앱 매니페스트도 서명 검증" 카피). v1.x.
- **CHANGELOG.md** — 본 변경 1줄 추가는 G(verify+commit) 단계에서.

---

## 5. 테스트 invariant

본 sub-phase 변경은 *CI workflow만*이라 Rust 단위/통합 테스트 영향 0. 검증:

1. **paths trigger** — 3개 매니페스트 변경 시 모두 workflow 트리거 (수동 push로 확인).
2. **sign step idempotent** — 동일 본문이면 .minisig byte-identical → commit skip.
3. **verify self-check** — pubkey 설정 시 모든 3개 파일 verify 통과.
4. **secret 미설정 graceful** — `CATALOG_MINISIGN_SECRET_KEY` 비어있으면 workflow는 success + warning만.
5. **partial 매니페스트** — 3종 중 1개만 변경 시에도 loop가 모두 처리 (idempotent — 미변경 .minisig도 같음).
6. **commit message `[skip-sign]`** — 자기 자신 commit이 다음 트리거에 들어가지 않게.

CI 자체 검증은 main push 후 GitHub Actions UI 모니터링.

---

## 6. 다음 페이즈 인계

### 진입 조건 (v1.x 13'.g.4)
- 본 sub-phase의 .minisig 생성이 main에서 정상 동작 확인.
- CATALOG_MINISIGN_SECRET_KEY / CATALOG_MINISIGN_PASSWORD / CATALOG_MINISIGN_PUBKEY secret 등록 (사용자 결정).
- LMMASTER_CATALOG_PUBKEY env (또는 별도 secret) 빌드 시 임베드 결정.

### Phase 13'.g.4 작업 항목
1. `apps/desktop/src-tauri/src/lib.rs` (또는 적절한 위치) — `verify_app_manifest_signature` IPC 추가, 일반화.
2. registry-fetcher 호출 측 — fetch 후 verify 호출 → 실패 시 BundledMissing 또는 한국어 에러.
3. UX — Settings에 "매니페스트 서명 검증" 토글 (기본 ON, opt-out 가능). Diagnostics에 검증 상태 카드.
4. ADR-0047 v2 또는 ADR-0059 — 정식 확장 결정.
5. 통합 테스트 6+ 가드 — verify 성공/실패/secret 미설정/manifest 미존재/sig 미존재/timeout.
6. EULA 한국어/영어 카피 갱신.

### 위험
- 매니페스트 변경 빈번 → 매번 .minisig commit → main 히스토리 noise. 완화: `[skip-sign]` 자기 자신 차단 + commit 단일 batch.
- secret 누출 시 모든 매니페스트 위변조 가능. 완화: 키 회전 정책 (90일 overlap, ADR-0047).

---

## 출처

- ADR-0047 (`docs/adr/0047-minisign-catalog-signature.md`)
- Phase 13'.g.2.d 결정 노트 (`docs/research/phase-13pg-catalog-signature-decision.md`)
- ADR-0054 (`docs/adr/0054-cache-signature-verified-marker.md`)
- registry-fetcher signature.rs / lib.rs (Phase 13'.g.2.a~c)
