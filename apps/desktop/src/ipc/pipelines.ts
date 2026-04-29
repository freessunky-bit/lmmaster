// Pipelines IPC — Phase 6'.c. Settings 화면이 게이트웨이 필터를 토글하고 감사 로그를
// 살펴볼 때 호출하는 5개 Tauri command 래퍼.
//
// 정책 (ADR-0025, phase-6p-updater-pipelines-decision.md §6):
// - Backend의 PipelinesApiError는 invoke().reject로 도달 — kind discriminant 기반 narrow.
// - audit log의 timestamp_iso는 RFC3339 string. JS Date 호환.
// - listPipelines는 i18n 비의존 한국어 fallback도 포함. UI는 i18n 키를 우선 사용.

import { invoke } from "@tauri-apps/api/core";

// ── Backend 미러 타입 ─────────────────────────────────────────────────

/** Rust pipelines::PipelineDescriptor 미러. */
export interface PipelineDescriptor {
  /** "pii-redact" | "token-quota" | "observability" — UI는 이 id로 i18n 키 분기. */
  id: string;
  /** Backend가 보장하는 한국어 fallback 라벨. UI는 i18n 키 우선. */
  display_name_ko: string;
  /** Backend가 보장하는 한국어 fallback 설명. */
  description_ko: string;
  /** 현재 enabled 상태 (영속 config 반영). */
  enabled: boolean;
}

/** Rust pipelines::PipelinesConfig 미러. */
export interface PipelinesConfig {
  pii_redact_enabled: boolean;
  token_quota_enabled: boolean;
  observability_enabled: boolean;
}

/** Rust pipelines::AuditEntryDto 미러. */
export interface AuditEntry {
  /** "pii-redact" | "token-quota" | "observability" 등. */
  pipeline_id: string;
  /** "passed" | "modified" | "blocked". */
  action: string;
  /** RFC3339 ISO. */
  timestamp_iso: string;
  /** Pipeline이 남긴 추가 설명 (변경 횟수 등). */
  details: string | null;
}

/** invoke().reject로 도달하는 backend 에러. */
export type PipelinesApiError =
  | { kind: "unknown-pipeline"; pipeline_id: string }
  | { kind: "persist-failed"; message: string };

// ── Tauri command 래퍼 ───────────────────────────────────────────────

/** v1 시드 3종 Pipeline 설명자 + enabled 상태. */
export async function listPipelines(): Promise<PipelineDescriptor[]> {
  return invoke<PipelineDescriptor[]>("list_pipelines");
}

/** 단일 Pipeline 토글 + 영속. 알 수 없는 id는 backend가 한국어 에러로 거부. */
export async function setPipelineEnabled(
  pipelineId: string,
  enabled: boolean,
): Promise<void> {
  await invoke<void>("set_pipeline_enabled", {
    pipelineId,
    enabled,
  });
}

/** 현재 영속 config snapshot. */
export async function getPipelinesConfig(): Promise<PipelinesConfig> {
  return invoke<PipelinesConfig>("get_pipelines_config");
}

/**
 * 마지막 N개 감사 entry — 시간 역순(최신부터). limit 기본 50, 최대 200.
 *
 * @param limit 조회할 개수. 미지정 시 backend default(50). 200 초과 시 backend에서 clamp.
 */
export async function getAuditLog(limit?: number): Promise<AuditEntry[]> {
  return invoke<AuditEntry[]>("get_audit_log", {
    limit: limit ?? null,
  });
}

/** 감사 ring buffer 비우기. 영속 config는 영향 없음. */
export async function clearAuditLog(): Promise<void> {
  await invoke<void>("clear_audit_log");
}
