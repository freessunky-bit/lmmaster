//! Deterministic recommender (ADR-0013, Phase 2'.a 보강).
//!
//! 입력: HostFingerprint + Catalog(매니페스트 entries) + 타겟 카테고리.
//! 출력: best/balanced/lightweight/fallback + ExclusionReason 리스트.
//!
//! 보정 5종 (Phase 2'.a 결정 노트 §0):
//! 1. Headroom bonus — VRAM 1.3× 이상이면 +5.
//! 2. Asymmetric category match — Same(+20) / Adjacent(+5) / Other(0).
//! 3. Lexicographic tie-breaker — (score desc, maturity desc, install_size asc, id asc).
//! 4. Lightweight cliff prevention — install_size_mb ≤ 5000만 lightweight 후보.
//! 5. ExclusionReason enum — 자유 문자열 금지.

use serde::{Deserialize, Serialize};
use shared_types::{HostFingerprint, IntentId, ModelCategory};

use crate::manifest::{Maturity, ModelEntry, ModelPurpose, VerificationTier};

const LIGHTWEIGHT_MAX_MB: u64 = 5000;
const HEADROOM_RATIO_NUM: u64 = 13;
const HEADROOM_RATIO_DEN: u64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Recommendation {
    pub best_choice: Option<String>,
    pub balanced_choice: Option<String>,
    pub lightweight_choice: Option<String>,
    pub fallback_choice: Option<String>,
    pub excluded: Vec<ExclusionReason>,
    pub expected_tradeoffs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ExclusionReason {
    /// VRAM 부족.
    InsufficientVram {
        id: String,
        need_mb: u64,
        have_mb: u64,
    },
    /// RAM 부족.
    InsufficientRam {
        id: String,
        need_mb: u64,
        have_mb: u64,
    },
    /// 호환 가능한 런타임이 하나도 등록되지 않음 (현재는 모든 런타임이 후보 — 가용성 미확인이라 제외 안 함, 단 enum은 유지).
    IncompatibleRuntime { id: String },
    /// Maturity == Deprecated.
    Deprecated { id: String },
    /// Phase 13'.f.2 — purpose != general-chat (임베딩 / 베이스 / 재정렬).
    /// 채팅 추천에서 자동 제외. Workspace 또는 Workbench 별도 셀렉터에서 노출.
    PurposeMismatch { id: String, purpose: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CategoryDistance {
    Same,
    Adjacent,
    Other,
}

fn category_distance(a: ModelCategory, b: ModelCategory) -> CategoryDistance {
    if a == b {
        return CategoryDistance::Same;
    }
    use ModelCategory::*;
    let adjacent = matches!(
        (a, b),
        (AgentGeneral, Coding)
            | (Coding, AgentGeneral)
            | (AgentGeneral, Roleplay)
            | (Roleplay, AgentGeneral)
            | (AgentGeneral, Slm)
            | (Slm, AgentGeneral)
    );
    if adjacent {
        CategoryDistance::Adjacent
    } else {
        CategoryDistance::Other
    }
}

fn maturity_score(m: Maturity) -> i32 {
    match m {
        Maturity::Stable => 10,
        Maturity::Beta => 5,
        Maturity::Experimental => 0,
        Maturity::Deprecated => -100,
    }
}

fn maturity_rank(m: Maturity) -> u8 {
    // 큰 값 = 더 좋음 — desc 정렬에서 stable이 먼저.
    match m {
        Maturity::Stable => 3,
        Maturity::Beta => 2,
        Maturity::Experimental => 1,
        Maturity::Deprecated => 0,
    }
}

#[derive(Debug, Clone)]
struct Scored {
    id: String,
    score: i32,
    maturity: Maturity,
    install_size_mb: u64,
}

fn evaluate(
    entry: &ModelEntry,
    host: &HostFingerprint,
    target: ModelCategory,
    intent: Option<&IntentId>,
) -> Result<Scored, ExclusionReason> {
    if entry.maturity == Maturity::Deprecated {
        return Err(ExclusionReason::Deprecated {
            id: entry.id.clone(),
        });
    }

    // Phase 13'.f.2 — purpose != general-chat 모델은 채팅 추천에서 자동 제외.
    // 임베딩(Retrieval)/베이스(FineTuneBase)/재정렬(Reranker) 모델이 chat target으로 선택되는 것 차단.
    if entry.purpose != ModelPurpose::GeneralChat {
        let purpose_label = match entry.purpose {
            ModelPurpose::FineTuneBase => "fine-tune-base",
            ModelPurpose::Retrieval => "retrieval",
            ModelPurpose::Reranker => "reranker",
            ModelPurpose::GeneralChat => unreachable!("matched in if above"),
        };
        return Err(ExclusionReason::PurposeMismatch {
            id: entry.id.clone(),
            purpose: purpose_label.into(),
        });
    }

    // VRAM 하드 컷.
    match (host.vram_mb, entry.min_vram_mb) {
        (Some(have), Some(min)) if have < min => {
            return Err(ExclusionReason::InsufficientVram {
                id: entry.id.clone(),
                need_mb: min,
                have_mb: have,
            });
        }
        (None, Some(min)) => {
            // GPU 미장착 + VRAM 요구 → 제외.
            return Err(ExclusionReason::InsufficientVram {
                id: entry.id.clone(),
                need_mb: min,
                have_mb: 0,
            });
        }
        _ => {}
    }

    // RAM 하드 컷.
    if host.ram_mb < entry.min_ram_mb {
        return Err(ExclusionReason::InsufficientRam {
            id: entry.id.clone(),
            need_mb: entry.min_ram_mb,
            have_mb: host.ram_mb,
        });
    }

    let mut s: i32 = 0;

    // 카테고리 (보정-2)
    s += match category_distance(entry.category, target) {
        CategoryDistance::Same => 20,
        CategoryDistance::Adjacent => 5,
        CategoryDistance::Other => 0,
    };

    // 한국어 우선 — language_strength × 2.
    s += entry.language_strength.unwrap_or(0) as i32 * 2;

    // 카테고리에 맞는 strength 가산.
    let cat_strength = match target {
        ModelCategory::Roleplay => entry.roleplay_strength,
        ModelCategory::Coding => entry.coding_strength,
        _ => entry.language_strength,
    };
    s += cat_strength.unwrap_or(0) as i32;

    // VRAM 적합도.
    if let (Some(have), Some(rec)) = (host.vram_mb, entry.rec_vram_mb) {
        if have >= rec {
            s += 30;
            // 보정-1 headroom bonus.
            if have >= rec.saturating_mul(HEADROOM_RATIO_NUM) / HEADROOM_RATIO_DEN {
                s += 5;
            }
        } else if have.saturating_mul(HEADROOM_RATIO_NUM) / HEADROOM_RATIO_DEN >= rec {
            s += 10; // tight, 작동은 함.
        }
    } else if entry.rec_vram_mb.is_none() {
        // CPU-friendly 모델 — GPU 없는 호스트에서 우대.
        if host.vram_mb.is_none() {
            s += 15;
        } else {
            s += 5;
        }
    }

    // RAM 적합도.
    if host.ram_mb >= entry.rec_ram_mb {
        s += 15;
    } else if host.ram_mb >= entry.min_ram_mb {
        s += 5;
    }

    // Maturity bias.
    s += maturity_score(entry.maturity);

    // Verified tier 가산.
    if entry.verification.tier == VerificationTier::Verified {
        s += 5;
    }

    // Phase 11'.b (ADR-0048) — Intent 가중.
    //   `domain_scores[intent] * 0.4` → 0..40 가산. 카테고리 Same(+20) 대비 우위.
    //   `intents.contains(intent)`이면 +5 (큐레이터 의도 표명).
    //   intent=None 또는 점수 미보유면 가산 0 (backward compat).
    if let Some(iid) = intent {
        if let Some(score) = entry.domain_scores.get(iid) {
            s += (*score * 0.4) as i32;
        }
        if entry.intents.iter().any(|t| t == iid) {
            s += 5;
        }
    }

    Ok(Scored {
        id: entry.id.clone(),
        score: s,
        maturity: entry.maturity,
        install_size_mb: entry.install_size_mb,
    })
}

/// Public 진입점 — 카테고리만 사용. 기존 caller 호환 wrapper.
///
/// `entries`는 카탈로그의 전체 엔트리(필터 전). 카테고리 필터링은 fitness 가중으로 수행.
pub fn compute(
    host: &HostFingerprint,
    target: ModelCategory,
    entries: &[ModelEntry],
) -> Recommendation {
    compute_with_intent(host, target, entries, None)
}

/// Public 진입점 — 카테고리 + 의도 가중. (Phase 11'.b, ADR-0048)
///
/// `intent`는 의도(intent picker) 신호 — `None`이면 기존 `compute(...)`와 동일 (backward compat).
/// `Some(iid)`이면 `domain_scores[iid]*0.4` + `intents.contains(iid)*5` 가산.
pub fn compute_with_intent(
    host: &HostFingerprint,
    target: ModelCategory,
    entries: &[ModelEntry],
    intent: Option<&IntentId>,
) -> Recommendation {
    let mut scored: Vec<Scored> = Vec::new();
    let mut excluded: Vec<ExclusionReason> = Vec::new();

    for entry in entries {
        match evaluate(entry, host, target, intent) {
            Ok(s) => scored.push(s),
            Err(reason) => excluded.push(reason),
        }
    }

    // 보정-3 lexicographic tie-breaker — (score desc, maturity desc, install_size asc, id asc).
    scored.sort_by_key(|s| {
        (
            std::cmp::Reverse(s.score),
            std::cmp::Reverse(maturity_rank(s.maturity)),
            s.install_size_mb,
            s.id.clone(),
        )
    });

    let best_choice = scored.first().map(|s| s.id.clone());

    // balanced — top 30% 안에서 install_size 중간값.
    let balanced_choice = if scored.is_empty() {
        None
    } else {
        let cutoff = ((scored.len() as f64 * 0.3).ceil() as usize).max(1);
        let mut top = scored.iter().take(cutoff).cloned().collect::<Vec<_>>();
        top.sort_by_key(|s| s.install_size_mb);
        let mid = top.len() / 2;
        Some(top[mid].id.clone())
    };

    // 보정-4 lightweight cliff.
    let lightweight_choice = scored
        .iter()
        .find(|s| s.install_size_mb <= LIGHTWEIGHT_MAX_MB)
        .map(|s| s.id.clone());

    // fallback — 항상 카탈로그 안에서 가장 작은 stable.
    let fallback_choice = entries
        .iter()
        .filter(|e| e.maturity == Maturity::Stable)
        .min_by_key(|e| e.install_size_mb)
        .map(|e| e.id.clone());

    let expected_tradeoffs = build_tradeoffs(&best_choice, &lightweight_choice, host);

    Recommendation {
        best_choice,
        balanced_choice,
        lightweight_choice,
        fallback_choice,
        excluded,
        expected_tradeoffs,
    }
}

fn build_tradeoffs(
    best: &Option<String>,
    lightweight: &Option<String>,
    host: &HostFingerprint,
) -> Vec<String> {
    let mut out = Vec::new();
    if best.is_none() {
        out.push("호스트에 맞는 모델을 찾지 못했어요. 사양을 확인해 주세요.".into());
    }
    if lightweight.is_none() && best.is_some() {
        out.push("가벼운 옵션은 5GB 이하 모델이 없어요.".into());
    }
    if host.vram_mb.is_none() {
        out.push(
            "GPU가 없으면 응답 속도가 느릴 수 있어요. CPU-friendly 모델을 우선 추천했어요.".into(),
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_distance_same() {
        assert_eq!(
            category_distance(ModelCategory::AgentGeneral, ModelCategory::AgentGeneral),
            CategoryDistance::Same
        );
    }

    #[test]
    fn category_distance_adjacent() {
        assert_eq!(
            category_distance(ModelCategory::AgentGeneral, ModelCategory::Coding),
            CategoryDistance::Adjacent
        );
        assert_eq!(
            category_distance(ModelCategory::Coding, ModelCategory::AgentGeneral),
            CategoryDistance::Adjacent
        );
    }

    #[test]
    fn category_distance_other() {
        assert_eq!(
            category_distance(ModelCategory::Coding, ModelCategory::Roleplay),
            CategoryDistance::Other
        );
    }

    #[test]
    fn maturity_score_ordering() {
        assert!(maturity_score(Maturity::Stable) > maturity_score(Maturity::Beta));
        assert!(maturity_score(Maturity::Beta) > maturity_score(Maturity::Experimental));
        assert!(maturity_score(Maturity::Experimental) > maturity_score(Maturity::Deprecated));
    }

    #[test]
    fn exclusion_reason_serializes_with_kind_tag() {
        let r = ExclusionReason::InsufficientVram {
            id: "x".into(),
            need_mb: 8000,
            have_mb: 4000,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["kind"], "insufficient-vram");
        assert_eq!(v["id"], "x");
    }
}
