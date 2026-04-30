// HuggingFace 하이브리드 검색 IPC — Phase 11'.c (ADR-0049).
//
// 정책:
// - SSOT는 Rust 측 hf_search.rs. TS는 단순 미러.
// - "지원 외" 라벨로 큐레이션 thesis(ADR-0049 §A 거부) 보존.
// - 한국어 graceful 에러 (HfSearchError kebab-case kind).

import { invoke } from "@tauri-apps/api/core";

import type { CustomModel } from "./workbench";

export interface HfSearchHit {
  /** `org/name` HF repo 경로. */
  repo: string;
  downloads: number;
  likes: number;
  /** RFC3339 — 빈 문자열 가능. */
  last_modified: string;
  pipeline_tag?: string | null;
  library_name?: string | null;
}

export type HfSearchError =
  | { kind: "network"; message: string }
  | { kind: "parse"; message: string }
  | { kind: "upstream"; status: number };

/**
 * HuggingFace Hub 검색. 빈 query는 즉시 빈 결과 (네트워크 호출 X).
 * 실패 시 Rust HfSearchError가 한국어 메시지로 reject.
 */
export async function searchHfModels(query: string): Promise<HfSearchHit[]> {
  return invoke<HfSearchHit[]>("search_hf_models", { query });
}

/**
 * "지금 시도해 볼게요" 흐름 — HF 검색 결과 모델을 사용자 PC의 CustomModelRegistry에 등록.
 *
 * 등록된 모델은 CustomModelsSection에 노출되며, modelfile에 자동 워닝이 prepend됨.
 * (큐레이션 외 모델 — chat template/quantization 사용자가 검증).
 */
export async function registerHfModel(
  repo: string,
  file?: string,
): Promise<CustomModel> {
  return invoke<CustomModel>("register_hf_model", { repo, file });
}

/**
 * "큐레이션 추가 요청" CTA → GitHub Issue prefilled URL.
 *
 * `tauri::api::shell::open(url)`로 시스템 브라우저 open — 외부 통신 0 정책상 자동 POST는 거부.
 * 본 함수는 URL string만 생성. Rust hf_search::curation_request_url과 SSOT 동일성은
 * 둘 다 동일 GitHub repo + 템플릿명을 참조하므로 단위 테스트로 검증.
 */
export function curationRequestUrl(repo: string): string {
  const title = `[큐레이션 요청] ${repo}`;
  const body =
    "## 모델 정보\n\n" +
    `- HuggingFace repo: https://huggingface.co/${repo}\n\n` +
    "## 사용 의도\n\n" +
    '(어떤 작업에 쓰고 싶은지 적어주세요 — 예: "한국어 코딩", "비전 + 한국어")\n\n' +
    "## 추가 메모\n\n(선택)\n";
  const params = new URLSearchParams({
    template: "curation-request.yml",
    title,
    body,
  });
  return `https://github.com/freessunky-bit/lmmaster/issues/new?${params}`;
}
