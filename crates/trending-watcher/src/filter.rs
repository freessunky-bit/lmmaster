//! Deterministic 필터 매트릭스 — Phase 21'.c.
//!
//! 정책 (ADR-0059 §4):
//!
//! ```text
//! score = 0.35·norm(Open_LLM_Avg)
//!       + 0.20·log10(downloads_30d)
//!       + 0.20·korean_signal
//!       + 0.15·license_score
//!       + 0.10·gguf_present
//! ```
//!
//! - License score: apache-2/mit = 1.0, llama3.x-community/gemma = 0.7, exaone/nvidia-open = 0.4, *other = 0.0 (자동 제외)*.
//! - Korean signal: cardData.language=ko 1.0 / 본문 정규식 hit (0~3) → 0.3·count cap 1.0 / 미언급 0.0.
//! - GGUF present: library_name=gguf OR `unsloth|bartowski|lmstudio-community|TheBloke|MaziyarPanahi` 미러 hit = 1.0.
//! - 사이즈 게이트: 3B~14B만 정식 큐 (외이는 info-only).
//! - 다운로드 임계: ≥ 1k 1차, ≥ 10k 추천.
//!
//! 모든 가중치/임계는 코드 상수. 단위 테스트 invariant — 동일 입력 100회 동일 score.

use crate::source::{HfModel, OpenLlmRow};

/// 가중치 (ADR-0059 §4 매트릭스).
pub const WEIGHT_OPEN_LLM: f64 = 0.35;
pub const WEIGHT_DOWNLOADS: f64 = 0.20;
pub const WEIGHT_KOREAN: f64 = 0.20;
pub const WEIGHT_LICENSE: f64 = 0.15;
pub const WEIGHT_GGUF: f64 = 0.10;

/// 사이즈 게이트 (B 단위).
pub const SIZE_MIN_B: f64 = 3.0;
pub const SIZE_MAX_B: f64 = 14.0;

/// 다운로드 임계.
pub const DOWNLOADS_MIN: u64 = 1_000;

/// 한 후보 모델의 score 산출 결과.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub hub_id: String,
    pub score: f64,
    pub reasons: Vec<String>,
    pub disposition: Disposition,
}

/// 큐 분류 — review queue / info-only / reject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Disposition {
    /// 큐레이터 알림 issue 자동 생성 (정식 review queue).
    Queue,
    /// 사이즈 외 / 다운로드 미달 — info-only (별도 라벨).
    InfoOnly,
    /// 라이선스 X 등 자동 제외.
    Reject,
}

/// 라이선스 → score 매핑.
pub fn license_score(license: &str) -> f64 {
    match license.to_ascii_lowercase().as_str() {
        "apache-2.0" | "mit" | "bsd-2-clause" | "bsd-3-clause" => 1.0,
        "llama3" | "llama3.1" | "llama3.2" | "llama3.3" | "llama-community" | "gemma" => 0.7,
        "exaone" | "nvidia-open-model" | "exaone-custom" => 0.4,
        _ => 0.0,
    }
}

/// 한국어 signal — tags + cardData.language (HF model).
///
/// - `tags`에 `ko` / `korean` / `한국어` 포함 → 1.0 (단순 매칭).
/// - 그 외 — 본 함수는 보수적 0.0. v1.x.b에 model card 본문 정규식 추가 가능.
pub fn korean_signal(tags: &[String]) -> f64 {
    let needles = [
        "ko",
        "korean",
        "한국어",
        "한글",
        "exaone",
        "hyperclovax",
        "hyperclova",
        "hcx",
    ];
    let lower: Vec<String> = tags.iter().map(|s| s.to_ascii_lowercase()).collect();
    let hits = needles
        .iter()
        .filter(|n| lower.iter().any(|tag| tag.contains(*n)))
        .count();
    if hits == 0 {
        return 0.0;
    }
    // hits 1 → 0.6, hits 2 → 0.9, hits 3+ → 1.0 cap (다양한 신호 누적 시 높임).
    (0.3 * hits as f64).clamp(0.6, 1.0)
}

/// GGUF 변종 존재 여부 — library_name 또는 author 미러.
pub fn gguf_present(model: &HfModel) -> f64 {
    if model.library_name.as_deref() == Some("gguf") {
        return 1.0;
    }
    let mirror_authors = [
        "unsloth/",
        "bartowski/",
        "lmstudio-community/",
        "thebloke/",
        "maziyarpanahi/",
    ];
    let id_lower = model.id.to_ascii_lowercase();
    if mirror_authors
        .iter()
        .any(|prefix| id_lower.contains(prefix))
    {
        return 1.0;
    }
    0.0
}

/// Open LLM Average → 0.0..1.0 정규화. 100점 만점.
pub fn normalize_open_llm_avg(avg: f64) -> f64 {
    (avg / 100.0).clamp(0.0, 1.0)
}

/// 다운로드 → log10 정규화. 1k=0.0, 1M=1.0 cap.
pub fn normalize_downloads(downloads: u64) -> f64 {
    if downloads == 0 {
        return 0.0;
    }
    let log = (downloads as f64).log10();
    // 1000 (10^3) = 0.0 / 1_000_000 (10^6) = 1.0 / 그 이상 cap.
    ((log - 3.0) / 3.0).clamp(0.0, 1.0)
}

/// 한 후보의 score 산출 — Phase 21'.c 핵심 함수.
pub fn score_candidate(model: &HfModel, leaderboard: Option<&OpenLlmRow>) -> Scored {
    let mut reasons: Vec<String> = Vec::new();

    let lic = leaderboard
        .and_then(|r| r.license.as_deref())
        .unwrap_or_default();
    let lic_score = license_score(lic);
    let kor = korean_signal(&model.tags);
    let gguf = gguf_present(model);
    let dl_norm = normalize_downloads(model.downloads);
    let lb_norm = leaderboard
        .map(|r| normalize_open_llm_avg(r.avg))
        .unwrap_or(0.0);

    let score = WEIGHT_OPEN_LLM * lb_norm
        + WEIGHT_DOWNLOADS * dl_norm
        + WEIGHT_KOREAN * kor
        + WEIGHT_LICENSE * lic_score
        + WEIGHT_GGUF * gguf;

    // disposition 결정 — 라이선스 X 자동 제외 우선.
    if lic_score == 0.0 {
        reasons.push(format!("라이선스 거부: {lic}"));
        return Scored {
            hub_id: model.id.clone(),
            score,
            reasons,
            disposition: Disposition::Reject,
        };
    }

    let mut info_only = false;
    if let Some(row) = leaderboard {
        if row.params_b > 0.0 && (row.params_b < SIZE_MIN_B || row.params_b > SIZE_MAX_B) {
            reasons.push(format!(
                "사이즈 범위 외: {:.1}B (정식 큐 3~14B)",
                row.params_b
            ));
            info_only = true;
        }
    }
    if model.downloads < DOWNLOADS_MIN {
        reasons.push(format!(
            "다운로드 미달: {} (임계 {DOWNLOADS_MIN})",
            model.downloads
        ));
        info_only = true;
    }

    if info_only {
        return Scored {
            hub_id: model.id.clone(),
            score,
            reasons,
            disposition: Disposition::InfoOnly,
        };
    }

    reasons.push(format!(
        "통과 (avg={lb_norm:.2}, dl={dl_norm:.2}, ko={kor:.2}, lic={lic_score:.2}, gguf={gguf:.2})"
    ));
    Scored {
        hub_id: model.id.clone(),
        score,
        reasons,
        disposition: Disposition::Queue,
    }
}

/// 후보 batch score + Queue 분류만 결과 반환 (sorted desc).
pub fn rank_candidates(models: &[HfModel], leaderboard: &[OpenLlmRow]) -> Vec<Scored> {
    let mut all: Vec<Scored> = models
        .iter()
        .map(|m| {
            let lb = leaderboard.iter().find(|r| {
                // hub_id 매칭 — 정확 또는 normalized.
                r.model.eq_ignore_ascii_case(&m.id)
                    || r.model
                        .split('/')
                        .next_back()
                        .map(|s| s.eq_ignore_ascii_case(m.id.split('/').next_back().unwrap_or("")))
                        .unwrap_or(false)
            });
            score_candidate(m, lb)
        })
        .collect();

    // 정렬 deterministic — score desc, 동점 시 hub_id asc.
    all.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.hub_id.cmp(&b.hub_id))
    });
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_hf(id: &str, downloads: u64, tags: Vec<&str>, library: Option<&str>) -> HfModel {
        HfModel {
            id: id.into(),
            downloads,
            likes: 0,
            trending_score: 0.0,
            pipeline_tag: Some("text-generation".into()),
            tags: tags.into_iter().map(String::from).collect(),
            library_name: library.map(String::from),
            gated: serde_json::Value::Null,
        }
    }

    fn sample_lb(model: &str, avg: f64, params_b: f64, license: &str) -> OpenLlmRow {
        OpenLlmRow {
            model: model.into(),
            avg,
            params_b,
            license: Some(license.into()),
            chat_template: true,
        }
    }

    // ── license_score (Phase 21'.a 보존) ─────────────────────────────

    #[test]
    fn license_score_apache_or_mit_full() {
        assert_eq!(license_score("apache-2.0"), 1.0);
        assert_eq!(license_score("MIT"), 1.0);
    }

    #[test]
    fn license_score_llama_community_partial() {
        assert_eq!(license_score("llama3.1"), 0.7);
        assert_eq!(license_score("gemma"), 0.7);
    }

    #[test]
    fn license_score_exaone_lower() {
        assert_eq!(license_score("exaone"), 0.4);
        assert_eq!(license_score("nvidia-open-model"), 0.4);
    }

    #[test]
    fn license_score_unknown_zero_excluded() {
        assert_eq!(license_score("CC-BY-NC-4.0"), 0.0);
        assert_eq!(license_score(""), 0.0);
    }

    // ── korean_signal ────────────────────────────────────────────────

    #[test]
    fn korean_signal_zero_no_match() {
        let s = korean_signal(&["transformers".into(), "english".into()]);
        assert_eq!(s, 0.0);
    }

    #[test]
    fn korean_signal_one_hit() {
        let s = korean_signal(&["korean".into()]);
        assert!((s - 0.6).abs() < 0.001);
    }

    #[test]
    fn korean_signal_multi_hit_capped() {
        let s = korean_signal(&[
            "ko".into(),
            "korean".into(),
            "EXAONE".into(),
            "한국어".into(),
        ]);
        assert!((s - 1.0).abs() < 0.001);
    }

    // ── gguf_present ─────────────────────────────────────────────────

    #[test]
    fn gguf_present_library_name() {
        let m = sample_hf("foo/bar", 100, vec![], Some("gguf"));
        assert_eq!(gguf_present(&m), 1.0);
    }

    #[test]
    fn gguf_present_mirror_author() {
        let m = sample_hf("bartowski/SomeModel-GGUF", 100, vec![], None);
        assert_eq!(gguf_present(&m), 1.0);
    }

    #[test]
    fn gguf_present_neither() {
        let m = sample_hf("foo/bar", 100, vec![], Some("transformers"));
        assert_eq!(gguf_present(&m), 0.0);
    }

    // ── normalize ────────────────────────────────────────────────────

    #[test]
    fn normalize_open_llm_avg_clamps() {
        assert_eq!(normalize_open_llm_avg(0.0), 0.0);
        assert_eq!(normalize_open_llm_avg(50.0), 0.5);
        assert_eq!(normalize_open_llm_avg(150.0), 1.0);
    }

    #[test]
    fn normalize_downloads_log_scale() {
        assert_eq!(normalize_downloads(0), 0.0);
        // 1000 = 10^3 → 0.0
        assert!(normalize_downloads(1000) < 0.01);
        // 1_000_000 = 10^6 → 1.0
        assert!(normalize_downloads(1_000_000) >= 0.99);
        assert_eq!(normalize_downloads(10_000_000), 1.0);
    }

    // ── score_candidate ───────────────────────────────────────────────

    #[test]
    fn score_candidate_apache_korean_gguf_passes_queue() {
        let m = sample_hf(
            "test/qwen3-korean",
            10_000,
            vec!["korean", "gguf"],
            Some("gguf"),
        );
        let lb = sample_lb("test/qwen3-korean", 67.0, 7.0, "apache-2.0");
        let s = score_candidate(&m, Some(&lb));
        assert_eq!(s.disposition, Disposition::Queue);
        assert!(s.score > 0.5);
    }

    #[test]
    fn score_candidate_unknown_license_rejected() {
        let m = sample_hf("test/cc-bync", 100_000, vec!["korean"], Some("gguf"));
        let lb = sample_lb("test/cc-bync", 70.0, 7.0, "CC-BY-NC-4.0");
        let s = score_candidate(&m, Some(&lb));
        assert_eq!(s.disposition, Disposition::Reject);
    }

    #[test]
    fn score_candidate_low_downloads_info_only() {
        let m = sample_hf("test/tiny", 500, vec!["korean"], Some("gguf"));
        let lb = sample_lb("test/tiny", 50.0, 7.0, "apache-2.0");
        let s = score_candidate(&m, Some(&lb));
        assert_eq!(s.disposition, Disposition::InfoOnly);
    }

    #[test]
    fn score_candidate_too_large_info_only() {
        let m = sample_hf("test/big", 50_000, vec![], Some("gguf"));
        let lb = sample_lb("test/big", 70.0, 70.0, "apache-2.0");
        let s = score_candidate(&m, Some(&lb));
        assert_eq!(s.disposition, Disposition::InfoOnly);
    }

    // ── deterministic invariant (CLAUDE.md §4.4) ──────────────────────

    #[test]
    fn score_candidate_deterministic_100x() {
        let m = sample_hf("test/det", 50_000, vec!["korean"], Some("gguf"));
        let lb = sample_lb("test/det", 67.5, 7.0, "apache-2.0");
        let first = score_candidate(&m, Some(&lb));
        for _ in 0..100 {
            let next = score_candidate(&m, Some(&lb));
            assert_eq!(first, next, "score_candidate must be deterministic");
        }
    }

    #[test]
    fn rank_candidates_sorts_desc_with_id_tiebreaker() {
        let models = vec![
            sample_hf("z/zeta", 10_000, vec!["korean"], Some("gguf")),
            sample_hf("a/alpha", 10_000, vec!["korean"], Some("gguf")),
        ];
        let lb = vec![
            sample_lb("z/zeta", 67.0, 7.0, "apache-2.0"),
            sample_lb("a/alpha", 67.0, 7.0, "apache-2.0"),
        ];
        let ranked = rank_candidates(&models, &lb);
        // 동점 — id alphabetic 순.
        assert_eq!(ranked[0].hub_id, "a/alpha");
        assert_eq!(ranked[1].hub_id, "z/zeta");
    }
}
