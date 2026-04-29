//! BenchReport / BenchKey / BenchAdapter trait — Phase 2'.c.
//!
//! 정책 (phase-2pc-bench-decision.md):
//! - 1차 metric: tg_tps + ttft_ms (카드 한 줄 요약).
//! - 2차 metric: pp_tps + e2e_ms + cold_load_ms (Drawer 상세).
//! - metrics_source enum — Native(Ollama) | WallclockEst(LMStudio).
//! - host_fingerprint_short = sha256(GPU+VRAM+RAM+OS)[:16] — 캐시 invalidate 키.

use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use shared_types::{HostFingerprint, RuntimeKind};

/// 단일 sample (한 호출의 native counter).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchSample {
    /// generation tokens/s — `eval_count / (eval_duration / 1e9)`.
    pub tg_tps: f64,
    /// prompt processing tokens/s — Ollama only.
    pub pp_tps: Option<f64>,
    /// 첫 응답 ms — wall-clock.
    pub ttft_ms: u32,
    /// 전체 응답 ms — wall-clock.
    pub e2e_ms: u32,
    /// 모델 load ms (warm 호출은 0에 가까움).
    pub load_ms: Option<u32>,
    /// 응답 첫 80자 (UI 미리보기용).
    pub sample_text_excerpt: Option<String>,
    /// 사용된 한국어 시드 id.
    pub prompt_id: String,
    /// 측정 출처 — Native(Ollama 5필드) | WallclockEst(LMStudio).
    pub metrics_source: BenchMetricsSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum BenchMetricsSource {
    /// Ollama eval_count/eval_duration 필드 사용 — 정확.
    Native,
    /// LM Studio usage 토큰 + wall-clock 추정 — 정확도 낮음.
    WallclockEst,
}

/// 측정 종합 보고서 — 캐시 디스크 직렬화 + IPC payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchReport {
    // ── 키 (캐시 invalidate) ────────────────────────────────────
    pub runtime_kind: RuntimeKind,
    pub model_id: String,
    pub quant_label: Option<String>,
    pub host_fingerprint_short: String,
    pub bench_at: SystemTime,
    pub digest_at_bench: Option<String>,

    // ── 1차 metric ──────────────────────────────────────────────
    pub tg_tps: f64,
    pub ttft_ms: u32,

    // ── 2차 metric ──────────────────────────────────────────────
    pub pp_tps: Option<f64>,
    pub e2e_ms: u32,
    pub cold_load_ms: Option<u32>,

    // ── 리소스 (Windows+NVIDIA만 정확) ─────────────────────────
    pub peak_vram_mb: Option<u32>,
    pub peak_ram_delta_mb: Option<u32>,

    // ── 메타 ────────────────────────────────────────────────────
    pub metrics_source: BenchMetricsSource,
    pub sample_count: u8,
    pub prompts_used: Vec<String>,
    pub timeout_hit: bool,
    pub sample_text_excerpt: Option<String>,
    pub took_ms: u64,

    // ── 진단 (실패 캐시 — 반복 시도 방지) ───────────────────────
    pub error: Option<BenchErrorReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum BenchErrorReport {
    /// 런타임 HTTP endpoint가 살아있지 않음.
    RuntimeUnreachable { message: String },
    /// 모델이 런타임에 등록되지 않음 (Ollama: `model not found`).
    ModelNotLoaded { model_id: String },
    /// VRAM 부족 — 매니페스트 정보 + 호스트 정보로 판단.
    InsufficientVram { need_mb: u32, have_mb: u32 },
    /// 사용자 cancel.
    Cancelled,
    /// 30초 절대 타임아웃 + 0 패스.
    Timeout,
    /// 그 외.
    Other { message: String },
}

/// 캐시 키 — `host_fingerprint_short` 계산은 host.rs 헬퍼가 담당.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BenchKey {
    pub runtime_kind: RuntimeKind,
    pub model_id: String,
    pub quant_label: Option<String>,
    pub host_fingerprint_short: String,
}

impl BenchKey {
    /// 디스크 파일명용 — `{runtime}-{slug}-{host}.json`.
    pub fn cache_file_name(&self) -> String {
        let runtime = match self.runtime_kind {
            RuntimeKind::Ollama => "ollama",
            RuntimeKind::LmStudio => "lmstudio",
            RuntimeKind::LlamaCpp => "llama-cpp",
            RuntimeKind::KoboldCpp => "kobold",
            RuntimeKind::Vllm => "vllm",
        };
        let slug = self
            .model_id
            .replace(['/', ':', '\\', ' '], "_")
            .replace("..", "_");
        format!("{runtime}-{slug}-{}.json", self.host_fingerprint_short)
    }
}

/// 단일 한국어 prompt 시드.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSeed {
    pub id: String,
    pub task: PromptTask,
    pub text: String,
    /// 목표 generation 토큰 수 — UI 표시용.
    pub target_tokens: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum PromptTask {
    Chat,
    Summary,
    Reasoning,
}

/// host_fingerprint_short — sha256-trunc(GPU+VRAM+RAM+OS)[:16].
///
/// 호스트가 바뀌면 캐시 자동 invalidate. 16자면 충돌 거의 0.
pub fn fingerprint_short(host: &HostFingerprint) -> String {
    use std::hash::{Hash, Hasher};
    // sha256 의존을 회피 — DefaultHasher로 충분 (충돌 위험 낮고 invalidate 트리거 용도).
    let mut h = std::collections::hash_map::DefaultHasher::new();
    host.os.hash(&mut h);
    host.gpu_vendor.hash(&mut h);
    host.gpu_model.hash(&mut h);
    host.vram_mb.hash(&mut h);
    host.ram_mb.hash(&mut h);
    format!("{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> HostFingerprint {
        HostFingerprint {
            os: "windows".into(),
            arch: "x86_64".into(),
            cpu: "test".into(),
            ram_mb: 16384,
            gpu_vendor: Some("nvidia".into()),
            gpu_model: Some("RTX 4090".into()),
            vram_mb: Some(24576),
        }
    }

    #[test]
    fn fingerprint_short_is_16_hex_chars() {
        let f = fingerprint_short(&host());
        assert_eq!(f.len(), 16);
        assert!(f.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn fingerprint_changes_when_gpu_changes() {
        let a = fingerprint_short(&host());
        let mut h2 = host();
        h2.gpu_model = Some("RTX 3090".into());
        let b = fingerprint_short(&h2);
        assert_ne!(a, b);
    }

    #[test]
    fn cache_file_name_normalizes_slashes_and_colons() {
        let key = BenchKey {
            runtime_kind: RuntimeKind::Ollama,
            model_id: "exaone-3.5-7.8b-instruct".into(),
            quant_label: Some("Q4_K_M".into()),
            host_fingerprint_short: "abcdef0123456789".into(),
        };
        let name = key.cache_file_name();
        assert_eq!(
            name,
            "ollama-exaone-3.5-7.8b-instruct-abcdef0123456789.json"
        );
    }

    #[test]
    fn cache_file_name_replaces_path_separators() {
        let key = BenchKey {
            runtime_kind: RuntimeKind::LmStudio,
            model_id: "vendor/model:Q4".into(),
            quant_label: None,
            host_fingerprint_short: "deadbeefdeadbeef".into(),
        };
        let name = key.cache_file_name();
        assert!(!name.contains('/'));
        assert!(!name.contains(':'));
        assert!(name.starts_with("lmstudio-"));
    }

    #[test]
    fn metrics_source_serializes_kebab() {
        let v = serde_json::to_value(BenchMetricsSource::WallclockEst).unwrap();
        assert_eq!(v, serde_json::json!("wallclock-est"));
    }

    #[test]
    fn bench_error_report_round_trip() {
        let e = BenchErrorReport::InsufficientVram {
            need_mb: 12000,
            have_mb: 6000,
        };
        let s = serde_json::to_value(&e).unwrap();
        assert_eq!(s["kind"], "insufficient-vram");
        let e2: BenchErrorReport = serde_json::from_value(s).unwrap();
        assert!(matches!(
            e2,
            BenchErrorReport::InsufficientVram {
                need_mb: 12000,
                have_mb: 6000
            }
        ));
    }
}
