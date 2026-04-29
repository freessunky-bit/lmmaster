// detect_environment Tauri command 래퍼 + 결과 타입 미러.
// 정책 (Phase 1A.4.b 보강 §2):
// - hardware-probe::HardwareReport + runtime-detector::DetectResult를 단일 invoke로 묶음.
// - 한국어 마법사가 직접 소비하는 형태 — 모든 enum은 kebab-case 일치.

import { invoke } from "@tauri-apps/api/core";

// ── Hardware report (crates/hardware-probe::types) ────────────────────

export type OsFamily = "windows" | "macos" | "linux" | "other";

export interface OsInfo {
  family: OsFamily;
  version: string;
  arch: string;
  kernel: string;
  rosetta?: boolean;
  distro?: string;
  distro_version?: string;
}

export interface CpuInfo {
  brand: string;
  vendor_id: string;
  physical_cores: number;
  logical_cores: number;
  frequency_mhz: number;
}

export interface MemInfo {
  total_bytes: number;
  available_bytes: number;
}

export type DiskKind = "ssd" | "hdd" | "other";

export interface DiskInfo {
  mount_point: string;
  kind: DiskKind;
  total_bytes: number;
  available_bytes: number;
}

export type GpuVendor =
  | "nvidia"
  | "amd"
  | "intel"
  | "apple"
  | "qualcomm"
  | "microsoft"
  | "other";

export interface GpuInfo {
  vendor: GpuVendor;
  name: string;
  vram_bytes?: number | null;
  driver?: string | null;
  cuda?: string | null;
  metal_family?: string | null;
}

export interface RuntimeInfo {
  webview2?: string | null;
  vc_redist_2022?: string | null;
  nvidia_driver?: string | null;
  cuda?: string | null;
  d3d12?: boolean | null;
  directml?: boolean | null;
  vulkan?: boolean | null;
  vulkan_devices?: unknown | null;
  metal?: boolean | null;
  glibc?: string | null;
  libstdcpp?: string | null;
}

export interface HardwareReport {
  os: OsInfo;
  cpu: CpuInfo;
  mem: MemInfo;
  disks: DiskInfo[];
  gpus: GpuInfo[];
  runtimes: RuntimeInfo;
  probed_at: string; // RFC3339
  probe_ms: number;
}

// ── Runtime detect (crates/runtime-detector) ─────────────────────────

export type RuntimeKind =
  | "llama-cpp"
  | "kobold-cpp"
  | "ollama"
  | "lm-studio"
  | "vllm";

export type RuntimeStatus = "running" | "installed" | "not-installed" | "error";

export interface DetectResult {
  runtime: RuntimeKind;
  status: RuntimeStatus;
  version?: string;
  endpoint?: string;
  error?: string;
}

// ── 통합 보고 ─────────────────────────────────────────────────────────

export interface EnvironmentReport {
  hardware: HardwareReport;
  runtimes: DetectResult[];
}

export type EnvApiError = { kind: "internal"; message: string };

/**
 * hardware-probe + runtime-detector 통합 호출. Promise reject 시 EnvApiError로 캐치 가능.
 * 일반적으로 1.0~2.5s 소요 (cold).
 */
export async function detectEnvironment(): Promise<EnvironmentReport> {
  return invoke<EnvironmentReport>("detect_environment");
}

// ── 작은 표현 헬퍼 ────────────────────────────────────────────────────

const GIB = 1024 * 1024 * 1024;

export function formatGiB(bytes: number, fractionDigits = 1): string {
  return `${(bytes / GIB).toFixed(fractionDigits)}GB`;
}

/** OS family를 한국어 친화 라벨로. */
export function osFamilyLabel(family: OsFamily): string {
  switch (family) {
    case "windows":
      return "Windows";
    case "macos":
      return "macOS";
    case "linux":
      return "Linux";
    default:
      return "기타";
  }
}

export function runtimeKindLabel(kind: RuntimeKind): string {
  switch (kind) {
    case "lm-studio":
      return "LM Studio";
    case "ollama":
      return "Ollama";
    case "llama-cpp":
      return "llama.cpp";
    case "kobold-cpp":
      return "KoboldCpp";
    case "vllm":
      return "vLLM";
  }
}
