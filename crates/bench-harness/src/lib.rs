//! crate: bench-harness — 30초 모델 벤치마크 (TTFT, throughput, peak memory).
//!
//! 정책 (ADR 후보 + Phase 2'.c 결정 노트):
//! - 기존 OllamaAdapter / LmStudioAdapter에 BenchAdapter trait 추가만.
//! - 30s 절대 타임아웃 + cooperative cancel. partial report 정책.
//! - Ollama native counter 1순위, LM Studio wallclock fallback (`metrics_source` 명시).
//! - 캐시 키 = (runtime, model, quant, host_fingerprint_short). TTL 30일.
//! - 한국어 prompt 시드 3종 — chat / summary / reasoning.

pub mod adapter;
pub mod cache;
pub mod error;
pub mod runner;
pub mod types;
pub mod workbench_responder;

pub use adapter::BenchAdapter;
pub use cache::{cache_path, invalidate, load_if_fresh, save, CacheError};
pub use error::BenchError;
pub use runner::{run_bench, BenchPlan, BENCH_BUDGET_SECS};
pub use types::{
    fingerprint_short, BenchErrorReport, BenchKey, BenchMetricsSource, BenchReport, BenchSample,
    PromptSeed, PromptTask,
};
pub use workbench_responder::{
    ResponderConfig, RuntimeKind as ResponderRuntimeKind, WorkbenchResponder,
};

/// 한국어 시드 3종 — `manifests/prompts/bench-ko.json`과 1:1 미러링.
/// 시드 파일이 없거나 빈 환경에서도 동작하도록 코드에 baseline 포함.
pub fn baseline_korean_seeds() -> Vec<PromptSeed> {
    vec![
        PromptSeed {
            id: "bench-ko-chat".into(),
            task: PromptTask::Chat,
            text: "안녕하세요. 오늘 점심은 뭘 추천해주실 수 있나요? 가볍게 먹을 수 있는 한식으로요.".into(),
            target_tokens: 50,
        },
        PromptSeed {
            id: "bench-ko-summary".into(),
            task: PromptTask::Summary,
            text: "다음 글을 두 문장으로 요약해 주세요.\n\n오픈소스 인공지능 모델은 클라우드 의존 없이 \
사용자 PC에서 동작하기 때문에 데이터 보안과 비용 측면에서 장점이 큽니다. \
그러나 모델 크기와 양자화 옵션을 잘못 고르면 성능이 크게 떨어질 수 있어 \
하드웨어에 맞는 모델을 추천해주는 도구가 중요합니다.".into(),
            target_tokens: 60,
        },
        PromptSeed {
            id: "bench-ko-reasoning".into(),
            task: PromptTask::Reasoning,
            text: "1, 1, 2, 3, 5, 8 다음 두 숫자는 무엇이고 왜 그럴까요? 단계별로 설명해 주세요.".into(),
            target_tokens: 100,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_seeds_have_three_korean_prompts() {
        let seeds = baseline_korean_seeds();
        assert_eq!(seeds.len(), 3);
        for s in &seeds {
            assert!(s
                .text
                .chars()
                .any(|c| (0xAC00..=0xD7A3).contains(&(c as u32))));
        }
    }

    #[test]
    fn baseline_seeds_cover_three_tasks() {
        let seeds = baseline_korean_seeds();
        let tasks: std::collections::HashSet<_> = seeds.iter().map(|s| s.task).collect();
        assert!(tasks.contains(&PromptTask::Chat));
        assert!(tasks.contains(&PromptTask::Summary));
        assert!(tasks.contains(&PromptTask::Reasoning));
    }
}
