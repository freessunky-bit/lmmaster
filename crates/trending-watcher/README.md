# trending-watcher (Phase 21'.a prototype)

> AI 모델 카탈로그 자동 큐레이션 — HuggingFace Trending + Open LLM Leaderboard 2 + Arena 미러 + KMMLU 정규식 → deterministic 필터 → 큐레이터 GitHub Issue 알림.
>
> 🇺🇸 [English](#english)

## 한국어

### 무엇을 하나요?

매일 6시간마다 (GHA cron) AI 모델 트렌드 사이트에서 *궤도에 오르고 평가 괜찮은 검증된 모델*을 자동 발견해요. **자동 추가는 X** — 큐레이터가 GitHub Issue로 알림 받고 직접 검토 후 LMmaster 본 repo에 manifest PR을 올려요.

### 정책 (ADR-0059 + Phase 21' 결정 노트)

- **외부 통신 화이트리스트**: `huggingface.co` + `github.com`만 (ADR-0026 정합).
- **Deterministic 필터**: LLM judge 0. 가중치 코드 상수.
- **큐레이션 정체성**: chat template 깨짐 / 라이선스 함정 / 한국어 자연스러움은 *큐레이터가 직접 검증*.

### 가중치 매트릭스

```
score = 0.35·norm(Open_LLM_Avg)
      + 0.20·log10(downloads_30d)
      + 0.20·korean_signal
      + 0.15·license_score
      + 0.10·gguf_present
```

- license_score: apache-2/mit (1.0) / llama3.x-community·gemma (0.7) / exaone·nvidia-open (0.4) / *other (0.0 — 자동 제외)*.
- korean_signal: `cardData.language=ko` (1.0) / 본문 `(한국어|Korean|한글|EXAONE|HyperCLOVA|HCX)` hit (0.3·count cap 1.0).
- 사이즈 게이트: 3B~14B만.
- 다운로드 임계: ≥ 1k.

### 큐레이터 흐름

1. GHA cron 6h → fetch + filter → report.md 출력
2. JasonEtco/create-an-issue dedupe → `[trending] <hub_id>` 제목 issue 생성/갱신
3. 큐레이터 검토 (chat template 한국어 발화 검증, 라이선스 약관, GGUF sha256)
4. 통과 → LMmaster 본 repo에 manifest PR (`manifests/snapshot/models/<cat>/<id>.json`)
5. PR merge → `node .claude/scripts/build-catalog-bundle.mjs` → catalog.json 갱신 → jsdelivr propagate → 사용자 카탈로그에 노출

### 현재 단계

**Phase 21'.a — prototype 골격 완성** (v0.0.3+).
- [x] Cargo crate 신설 + 모듈 골격 (source / filter / report)
- [x] license_score 함수 + 단위 테스트 4건
- [ ] Phase 21'.b — HF + Open LLM fetcher (실 reqwest 호출)
- [ ] Phase 21'.c — deterministic 필터 매트릭스 통합
- [ ] Phase 21'.d — GHA workflow + JasonEtco/create-an-issue
- [ ] Phase 21'.e — CURATION_GUIDE.md 통합 + 1주 운영 모니터링

### v2.x 계획

- 본 prototype 검증 후 **별도 repo `lmmaster-trending-watcher` (public, MIT)**로 분리 (ADR-0059 정공).
- LMmaster 본 repo는 PR 받는 측만.

---

<a id="english"></a>

## English

### What it does

Every 6 hours (GHA cron), discovers *trending and validated AI models* from public benchmark/leaderboard sources. **Never auto-merges** — the curator gets a GitHub Issue notification and reviews manually before opening a manifest PR in the LMmaster main repo.

### Policy (ADR-0059)

- **External whitelist**: `huggingface.co` + `github.com` only (per ADR-0026).
- **Deterministic filter**: zero LLM judge. All weights are code constants.
- **Curation identity**: chat template, license footguns, Korean fluency are *human-verified*.

### Weight matrix

```
score = 0.35·norm(Open_LLM_Avg)
      + 0.20·log10(downloads_30d)
      + 0.20·korean_signal
      + 0.15·license_score
      + 0.10·gguf_present
```

### Status

**Phase 21'.a — prototype scaffold** (v0.0.3+). See Korean section above for sub-phase roadmap.

---

## License

MIT OR Apache-2.0 (workspace dual). Same as LMmaster main.
