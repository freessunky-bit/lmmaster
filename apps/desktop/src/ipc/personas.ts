// 페르소나 시뮬 IPC — Personas-Korea 데이터셋 자동 다운로드 + 후속 (v0.8.x).
//
// v0.8.0: 데이터셋 상태 조회 + 자동 다운로드.
// v0.8.1+: 페르소나 정의·샘플링 / 설문 / 배치 실행 / 리포트.

import { Channel, invoke } from "@tauri-apps/api/core";

export interface PersonasDatasetStatus {
  installed: boolean;
  size_bytes: number;
  file_count: number;
}

export type PersonasDatasetEvent =
  | {
      kind: "status";
      status: string;
      file_index: number;
      file_total: number;
    }
  | {
      kind: "progress";
      completed_bytes: number;
      total_bytes: number;
      speed_bps: number;
    }
  | {
      kind: "completed";
      file_count: number;
      total_bytes: number;
    }
  | { kind: "failed"; message: string };

/** Personas-Korea 데이터셋이 캐시에 있는지 + 크기 정보. */
export async function getPersonasDatasetStatus(): Promise<PersonasDatasetStatus> {
  return invoke<PersonasDatasetStatus>("get_personas_dataset_status");
}

/** 데이터셋 자동 다운로드. Channel<PersonasDatasetEvent>로 진행 스트림. */
export async function downloadPersonasDataset(args: {
  onEvent: (e: PersonasDatasetEvent) => void;
}): Promise<void> {
  const channel = new Channel<PersonasDatasetEvent>();
  channel.onmessage = args.onEvent;
  return invoke<void>("download_personas_dataset", { channel });
}

// ── v0.8.1: 페르소나 샘플링 ──────────────────────────────────────

export interface PersonaFilter {
  sex?: string | null;
  age_min?: number | null;
  age_max?: number | null;
  province_includes?: string[];
  occupation_includes?: string[];
  keyword_includes?: string[];
  sample_size: number;
  seed?: number | null;
}

export interface Persona {
  uuid: string;
  sex: string;
  age: string;
  province: string;
  occupation: string;
  persona: string;
  fields: Record<string, string>;
}

export async function personasSample(filter: PersonaFilter): Promise<Persona[]> {
  return invoke<Persona[]>("personas_sample", { filter });
}

// ── v0.8.2: 설문 배치 실행 ──────────────────────────────────────

export interface SurveyQuestion {
  id: string;
  type: "single" | "multi" | "scale" | "open";
  text: string;
  options?: string[];
  scale?: string;
}

export interface SurveyDef {
  survey_id: string;
  title: string;
  questions: SurveyQuestion[];
}

export interface SurveyAnswer {
  persona_uuid: string;
  question_id: string;
  answer: string;
  took_ms: number;
  /** v0.8.4 — 응답이 토큰 한계로 잘렸으면 true. */
  truncated?: boolean;
}

// v0.8.4 — 사용자 조절 sampling 파라미터.
export interface SamplingParams {
  max_tokens?: number | null;
  temperature?: number | null;
  top_p?: number | null;
  repeat_penalty?: number | null;
  seed?: number | null;
}

export type FinishReason = "stop" | "length" | "aborted" | "meta" | "unknown";

export type PersonasSurveyEvent =
  | { kind: "started"; total_calls: number }
  | {
      kind: "progress";
      completed: number;
      total: number;
      current_persona: string;
      current_question: string;
    }
  | { kind: "answer"; answer: SurveyAnswer }
  | { kind: "completed"; count: number; total_ms: number }
  | { kind: "cancelled" }
  | { kind: "failed"; message: string };

export async function personasRunSurvey(args: {
  personas: Persona[];
  survey: SurveyDef;
  runtimeKind: "ollama" | "llama-cpp";
  modelId: string;
  systemExtra?: string;
  /** v0.8.4 — sampling 파라미터. None이면 어댑터 디폴트. */
  sampling?: SamplingParams | null;
  onEvent: (e: PersonasSurveyEvent) => void;
}): Promise<void> {
  const channel = new Channel<PersonasSurveyEvent>();
  channel.onmessage = args.onEvent;
  return invoke<void>("personas_run_survey", {
    args: {
      personas: args.personas,
      survey: args.survey,
      runtime_kind: args.runtimeKind,
      model_id: args.modelId,
      system_extra: args.systemExtra ?? null,
      sampling: args.sampling ?? null,
    },
    channel,
  });
}

// ── v0.8.3: 리포트 프롬프트 생성 ─────────────────────────────────

export type ReportStyle = "mckinsey" | "nielsen" | "academic";

export interface OptionCount {
  option: string;
  count: number;
}

export interface KeywordFreq {
  keyword: string;
  count: number;
}

export interface QuestionSummary {
  id: string;
  text: string;
  type: "single" | "multi" | "scale" | "open";
  option_counts?: OptionCount[];
  scale_mean?: number | null;
  open_samples?: string[];
  open_keyword_freq?: KeywordFreq[];
}

export interface ReportRequest {
  survey_title: string;
  persona_count: number;
  persona_distribution: string;
  question_summaries: QuestionSummary[];
  style: ReportStyle;
}

export async function personasGenerateReportPrompt(
  req: ReportRequest,
): Promise<string> {
  return invoke<string>("personas_generate_report_prompt", { req });
}

// ── v0.8.4: chunked map-reduce 리포트 ─────────────────────────────

export interface ReportPromptChunk {
  /** 1-based 순번. */
  seq: number;
  /** 총 청크 수. */
  total: number;
  /** 헤더 + 부분 데이터 + 부분 분석 지시 포함 prompt. */
  prompt: string;
  /** 휴리스틱 토큰 추정. */
  estimated_tokens: number;
}

export interface ReportPromptPlan {
  /** 청크 N개. total === 1이면 single-shot. */
  chunks: ReportPromptChunk[];
  /** 모든 청크 paste 후 사용자가 별도 발사할 종합 합성 prompt. 청크 1개면 null. */
  final_synthesis: string | null;
  /** UI 표시용 합산 토큰 추정. */
  estimated_tokens_total: number;
}

export async function personasGenerateReportPromptPlan(
  req: ReportRequest,
): Promise<ReportPromptPlan> {
  return invoke<ReportPromptPlan>("personas_generate_report_prompt_plan", {
    req,
  });
}
