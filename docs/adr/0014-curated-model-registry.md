# ADR-0014: 모델 레지스트리는 curated remote manifest + 로컬 cache

- Status: Accepted
- Date: 2026-04-26

## Context
정적 하드코딩 목록은 모델 트렌드를 따라가지 못한다. Hugging Face 같은 거대한 카탈로그를 그대로 노출하면 사용자가 길을 잃는다. "유력 모델을 카테고리별"로 보여줘야 하고 "유능한 결정"이 가능해야 한다.

## Decision
**Curated remote manifest**를 채택한다.
- 우리(또는 제휴 메인테이너)가 관리하는 manifest를 원격(HTTPS) 호스팅. JSON Lines 또는 단일 JSON 트리.
- 앱이 주기적/수동으로 sync해 로컬 cache(`workspace/manifests/`)에 저장.
- 카테고리(에이전트/캐릭터/코딩/사운드/SLM/embeddings/rerank)와 모델 메타(아래 필드)를 포함.
- 각 모델 엔트리는 source(HF repo / 공식 다운로드 URL)와 sha256, runner_compatibility를 명시.

모델 엔트리 최소 필드:
```
id, display_name, category, model_family, source,
runner_compatibility[], quantization_options[],
min_vram, rec_vram, min_ram, rec_ram, install_size,
context_guidance, language_strength, roleplay_strength, coding_strength,
tool_support, vision_support, structured_output_support,
license, maturity, portable_suitability, on_device_suitability,
fine_tune_suitability, notes, warnings
```

Recommender 출력:
```
best_choice, balanced_choice, lightweight_choice, fallback_choice,
excluded_choices_with_reason[], expected_tradeoffs
```

- Recommender는 **deterministic 점수 함수**(가중치 + 룰). Gemini가 추천하지 않는다(ADR-0013).
- Hugging Face Hub API는 **부가 메타 보강**(다운로드 수, 최근 업데이트)에만 사용. 1차 신뢰 소스는 우리 manifest.

## Consequences
- 카탈로그 품질을 우리가 관리.
- 사용자 PC 사양 기반 추천이 결정적.
- manifest 메인테너 비용 발생 — CI로 자동화(라이선스/사이즈 자동 추출, manual review 1회).
- 모델 저자 측 변경(파일 이동, 라이선스 변경)은 manifest 갱신으로 흡수.

## Alternatives considered
- **HF Hub 직접 노출**: 사용자 길 잃음. 거부.
- **하드코딩 목록**: 트렌드 미반영. 거부.
- **사용자 manifest 자유 추가**: 가능 옵션. v1.x에 advanced 설정으로 허용.

## References
- ADR-0013 (Gemini boundary)
