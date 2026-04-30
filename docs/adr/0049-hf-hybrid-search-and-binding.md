# ADR-0049 — HuggingFace 하이브리드 검색·바인딩

* **상태**: 채택 (2026-04-30) — Phase 11'.c 머지 완료. CustomModelsSection 패턴 재활용 + form-template 분리.
* **컨텍스트**: 사용자가 외부에서 들은 모델명("elyza/Llama-3-ELYZA-JP-8B 좋다더라")을 LMmaster에서 즉시 시도해 보고 싶어함. 기존 카탈로그는 큐레이터 검증 모델만 노출 → 사용자 자율성 제한 + "이 모델 추가해 주세요" 신호가 큐레이션에 도달하지 못함.
* **결정 노트**: `docs/research/phase-11p-12p-v1x-domain-axis-decision.md` §2.4

## 결정

1. **하이브리드 패턴 (LM Studio 차용)** — 큐레이션 모델이 1급, "지원 외" HF 모델은 명확히 구분된 별도 진입점.
2. HF Hub Search API 호출 (`GET https://huggingface.co/api/models?search=...`) — ADR-0026 §1 외부 통신 화이트리스트 기존 예외에 포함(jsDelivr/HF는 이미 예외).
3. 검색 결과는 노란 "지원 외" 배지 + downloads/likes 미리보기. 도메인 점수 표시 X.
4. "지금 시도해 볼게요" → CustomModelsSection 패턴 재활용 — 사용자 PC 등록, `notes`에 자동 워닝 prepend.
5. "큐레이션 추가 요청" → GitHub Issue URL을 시스템 브라우저 open(자동 POST 거부) + `.github/ISSUE_TEMPLATE/curation-request.yml` 신설.

## 근거

- **하이브리드 (C) vs 직접 바인딩 (A) vs 추가 요청만 (B)** — A는 thesis(큐레이션 차별점) 와해, B는 사용자 자율성 제한. C는 LM Studio 멘탈 모델과 정합 + 사용자 신호로 큐레이션 자라는 피드백 루프.
- **자동 GitHub Issue POST 거부** — 외부 통신 0 정책 + token 관리 비용. 시스템 브라우저 open이 사용자 클릭 보호 유지.
- **chat template 깨짐 휴리스틱** — 첫 응답에서 unicode replacement char 비율 측정 → 자동 워닝 (deterministic).

## 거부된 대안

- **A) 직접 바인딩(큐레이션 외 모델 1급 시민화)** — thesis 와해 + 책임 사용자 부담.
- **B) 추가 요청만(검색 결과는 미리보기만)** — 사용자가 즉시 시도 못함, 자율성 제한.
- **자동 GitHub API POST** — token 관리 + 외부 통신 0 위반.
- **HF 모델 자동 큐레이션 등록** — 검증 0 → chat template 깨짐 책임 모호.
- **자동 quant 매칭** — v2+. v1.x는 사용자 직접 선택(CustomModel 패턴).

## 결과 / 영향

- Catalog 검색바 옆 토글 (`📂 카탈로그 ⚪ HF에서 찾기`) 신설.
- HF 검색 결과 모달은 노란 ⚠ 배너 + "지원 외" 배지로 큐레이션과 명확 구분.
- 등록된 HF 모델은 CustomModelsSection 별도 섹션에 노출, 도메인 점수 미렌더.
- Issue 템플릿이 GitHub 큐레이션 신호 1차 채널.

## References

- 결정 노트: `docs/research/phase-11p-12p-v1x-domain-axis-decision.md` §2.4 + §3.4-5
- 관련 ADR: ADR-0026 (외부 통신 0 + 화이트리스트), ADR-0044 (live catalog), ADR-0048 (intent 축)
- 코드:
  - `apps/desktop/src-tauri/src/hf_search.rs` (신규 — Search API 호출)
  - `apps/desktop/src-tauri/src/hf_meta.rs` (재활용 — 메타 페치 패턴)
  - `apps/desktop/src/components/catalog/CustomModelsSection.tsx` (재활용 — 등록 UX 패턴)
  - `.github/ISSUE_TEMPLATE/curation-request.yml` (신규)
