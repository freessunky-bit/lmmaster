# Phase 2'.a — 카탈로그 + 추천 결정 노트

> 작성일: 2026-04-27
> 상태: 확정 (보강 리서치 후 5가지 알고리즘 보정 반영)

## 0. 결정 요약

1. **2-tier governance** — `Verified | Community`. 시드는 모두 `Community` 기본값.
   - 정책: `#[serde(default)] verification: VerificationInfo` 누락 시 `tier = Community`로 폴백.
   - v1에서는 cosmetic만 (UI 배지). v1.1+에서 서명 검증 채널로 확장.
2. **HfMeta 필드는 schema-now-data-later** — 스키마는 정의하지만 v1 시드는 비움. v1.1에서 HF Hub API → cache.rs로 채움.
3. **Deterministic recommender** — 같은 (HostFingerprint, Catalog snapshot) → 같은 출력. 5가지 보정 적용:
   - 보정-1 **Headroom bonus**: 호스트 VRAM이 모델 rec_vram의 1.3× 이상이면 +5점 (큰 모델 언락 인센티브).
   - 보정-2 **Asymmetric category match**: 같은 카테고리 +20점, 인접 카테고리(agent↔coding, agent↔roleplay) +5점, 그 외 0점. 같은 카테고리만 후보로 거르지 않고 전체 카탈로그 fitness에 가중치만 부여.
   - 보정-3 **Lexicographic tie-breaker**: 동점 시 `(maturity desc, install_size asc, id asc)` — Stable > Beta > Experimental, 작은 다운로드 우선, 최후 id 알파벳.
   - 보정-4 **Lightweight cliff prevention**: lightweight_choice는 install_size_mb ≤ 5000 (5GB 이하)로 강제. 부족하면 `None`.
   - 보정-5 **ExclusionReason enum** — 자유 문자열 대신 tagged enum (`InsufficientVram { need, have }`, `IncompatibleRuntime`, `Deprecated`, `WrongCategory`).
4. **Catalog API**: `Catalog::load_from_dir(path)` + `load_layered(snapshot, overlay)`로 bundled fallback과 사용자 다운로드본을 합친다. Overlay는 같은 id면 덮어쓰고, 새 id면 추가.
5. **8 시드 매니페스트** — Korean-first 우선:
   - **agent-general**: EXAONE 4.0 1.2B-Instruct (lightweight), EXAONE 3.5 7.8B-Instruct (balanced), HCX-SEED 8B (한국어 최강).
   - **roleplay**: Polyglot-Ko 12.8B (Korean RP).
   - **coding**: Qwen 2.5 Coder 3B (작은 한국어 OK), EXAONE 4.0 32B-Instruct (best, 큰 호스트).
   - **slm**: Llama 3.2 3B Instruct (영어 fallback).
   - **sound-stt**: Whisper Large v3 Korean.
6. **Tauri IPC**: `get_catalog(category: Option<ModelCategory>) -> CatalogView`, `get_recommendation(category: ModelCategory) -> Recommendation`. 둘 다 동기 (Catalog는 process-local Arc cache).
7. **테스트 fixture** — `host_low(8GB RAM, no GPU)`, `host_mid(16GB RAM, RTX 3060 12GB)`, `host_high(64GB RAM, RTX 4090 24GB)`, `host_tiny(4GB RAM)` × 결정성 invariant + id 충돌 + 잘못된 카테고리 폴백.

## 1. 리서치 요약 — 글로벌 엘리트 사례

### 1.1 Foundry Local (MS) — Hardware-aware filtering

- 카탈로그 매니페스트에 GPU vendor, VRAM 요구치, ONNX runtime을 명시.
- 호스트가 GPU 없으면 자동 CPU 변종으로 fallback.
- **차용**: `min_vram_mb` / `rec_vram_mb` + `runner_compatibility[]`로 동일 패턴.

### 1.2 Pinokio — 2-tier governance

- `verified: true/false`만 필드로 두고 UI에서 배지 표시. 검증 자체는 GitHub PR 워크플로.
- **차용**: `VerificationInfo { tier, verified_at, verified_by }` — v1은 cosmetic.

### 1.3 Hugging Face Hub API — Metadata API

- `https://huggingface.co/api/models/{repo}` → likes, downloads, lastModified.
- **차용**: `HfMeta { downloads, likes, last_modified }` 옵셔널. v1.1에서 cache로 갱신.

### 1.4 Cherry Studio — Assistant preset structure

- 모델 엔트리에 `use_case_examples: ["고객 상담 응답", "한국어 글쓰기"]` 식의 한국어 자연어 예시.
- **차용**: `use_case_examples: Vec<String>` — Workbench(Phase 5')에서 프롬프트 시드로 사용 예정.

### 1.5 Ollama Hub — Quantization tier

- 같은 모델의 Q4_K_M / Q5_K_M / Q8_0 옵션을 `quantization_options[]`로 묶음.
- **이미 채택**: `manifest.rs`의 `QuantOption { label, size_mb, sha256, file_path }`.

## 2. Recommender 알고리즘 명세

```text
fitness(model, host, target_category) ->
  let mut s = 0;

  // 카테고리 가중 (보정-2)
  s += match category_distance(model.category, target_category) {
      Same      => 20,
      Adjacent  => 5,
      Other     => 0,
  };

  // 한국어 우선 (Korean-first)
  s += model.language_strength.unwrap_or(0) as i32 * 2;

  // 호스트 적합 — VRAM
  match (host.vram_mb, model.rec_vram_mb) {
      (Some(have), Some(rec)) if have >= rec => {
          s += 30;
          // 보정-1 headroom bonus
          if have >= rec * 13 / 10 { s += 5; }
      }
      (Some(have), Some(rec)) if have * 13 / 10 >= rec => s += 10, // tight
      _ => {}
  }
  match (host.vram_mb, model.min_vram_mb) {
      (Some(have), Some(min)) if have < min =>
          return Excluded { model.id, InsufficientVram { need: min, have } },
      _ => {}
  }

  // RAM
  if host.ram_mb >= model.rec_ram_mb { s += 15; }
  else if host.ram_mb >= model.min_ram_mb { s += 5; }
  else { return Excluded { model.id, InsufficientRam { ... } }; }

  // Maturity bias
  s += match model.maturity {
      Stable => 10, Beta => 5, Experimental => 0, Deprecated => -100,
  };

  // 한국어 카탈로그 — verified tier 가산
  if model.verification.tier == Verified { s += 5; }

  s
```

`compute(host, target_category, catalog) -> Recommendation`:
1. 카탈로그 전체에 대해 `fitness` 계산.
2. `Excluded` 항목 분리.
3. 남은 항목 정렬: `(score desc, maturity desc, install_size asc, id asc)` — 보정-3.
4. **best_choice** = 1순위 (없으면 None).
5. **balanced_choice** = score top 30% 안에서 install_size_mb 중간값 (median).
6. **lightweight_choice** = install_size_mb ≤ 5000인 1순위. 보정-4로 cliff 회피.
7. **fallback_choice** = 항상 Bundled tier에 있는 가장 작은 stable 모델 (Llama 3.2 3B).

## 3. JSON 매니페스트 스키마 (확장)

```jsonc
{
  "schema_version": 1,
  "generated_at": "2026-04-27T00:00:00Z",
  "entries": [
    {
      "id": "exaone-4.0-1.2b-instruct",
      "display_name": "EXAONE 4.0 1.2B Instruct",
      "category": "agent-general",
      "model_family": "exaone",
      "source": { "type": "hugging-face", "repo": "LGAI-EXAONE/EXAONE-4.0-1.2B-Instruct-GGUF", "file": "EXAONE-4.0-1.2B-Instruct-Q4_K_M.gguf" },
      "runner_compatibility": ["llama-cpp", "ollama", "lm-studio"],
      "quantization_options": [
        { "label": "Q4_K_M", "size_mb": 760, "sha256": "0000...0000", "file_path": "EXAONE-4.0-1.2B-Instruct-Q4_K_M.gguf" }
      ],
      "min_vram_mb": null,
      "rec_vram_mb": 2048,
      "min_ram_mb": 4096,
      "rec_ram_mb": 8192,
      "install_size_mb": 760,
      "context_guidance": "최대 32K context. 한국어 일상 대화/짧은 글쓰기.",
      "language_strength": 9,
      "roleplay_strength": 6,
      "coding_strength": 5,
      "tool_support": true,
      "vision_support": false,
      "structured_output_support": true,
      "license": "EXAONE Custom",
      "maturity": "stable",
      "portable_suitability": 9,
      "on_device_suitability": 9,
      "fine_tune_suitability": 6,
      "verification": { "tier": "verified", "verified_at": "2026-04-27", "verified_by": "lmmaster-curator" },
      "hf_meta": null,
      "use_case_examples": [
        "한국어 일상 대화",
        "짧은 글쓰기/요약",
        "온디바이스 비서"
      ],
      "warnings": []
    }
  ]
}
```

## 4. 미정 사항 / Phase 2'.b로 이월

- **30s 벤치마크 harness** — Phase 2'.c.
- **HF Hub API fetch** — v1.1 (Phase 6' 자동 갱신과 합칠 후보).
- **카탈로그 화면 + 카드 컴포넌트** — Phase 2'.b.
