// Telemetry IPC — Phase 7'.a.
//
// 정책 (ADR-0027 §5):
// - 기본 비활성. 사용자 명시 opt-in 후에만 익명 통계 전송 (실제 endpoint 연결은 Phase 7'.b).
// - 첫 opt-in 시 backend가 anonymous UUID + opted_in_at 발급.
// - 비활성 → 활성 토글 시 UUID 재사용 (PC 단위 고정 식별자, 개인 식별 X).

import { invoke } from "@tauri-apps/api/core";

/** Rust telemetry::TelemetryConfig 미러. */
export interface TelemetryConfig {
  enabled: boolean;
  /** 익명 UUID v4. opt-in 한 번이라도 했으면 보존돼요. */
  anon_id: string | null;
  /** 첫 opt-in RFC3339 ISO. */
  opted_in_at: string | null;
}

/** invoke().reject로 도달하는 backend 에러. */
export type TelemetryApiError =
  | { kind: "persist-failed"; message: string }
  | { kind: "internal"; message: string };

export async function getTelemetryConfig(): Promise<TelemetryConfig> {
  return invoke<TelemetryConfig>("get_telemetry_config");
}

export async function setTelemetryEnabled(
  enabled: boolean,
): Promise<TelemetryConfig> {
  return invoke<TelemetryConfig>("set_telemetry_enabled", { enabled });
}
