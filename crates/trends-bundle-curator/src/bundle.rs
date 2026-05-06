//! trends-bundle JSON 빌드 — Phase 22'.b → 22'.c.
//!
//! 정책 (ADR-0060 §1):
//! - schema_version: 1
//! - generated_at / expires_at (7일)
//! - curator_note_ko: 큐레이터 1~2문장 한국어 요약
//! - items: TrendItem (kind tagged enum 6종)
//! - minisign 서명: sign-catalog.yml `paths` trigger에 `manifests/apps/trends-bundle.json` 추가.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::source::TrendItem;

/// trends-bundle 합본 schema (ADR-0060 §1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrendsBundle {
    pub schema_version: u32,
    pub generated_at: String,
    pub expires_at: String,
    pub curator_note_ko: String,
    pub items: Vec<TrendItem>,
}

/// 시간 범위 — generated_at + 7일 = expires_at.
pub fn default_expires_at(generated: &str) -> String {
    // Phase 22'.c에서 실제 chrono::Duration::days(7) 적용.
    // 본 골격은 placeholder.
    format!("{generated} + 7d")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::TrendKind;

    fn sample_item() -> TrendItem {
        TrendItem {
            id: "test-id".into(),
            kind: TrendKind::Paper,
            title: "T".into(),
            summary_ko: "요약".into(),
            source: "src".into(),
            source_url: "https://x".into(),
            attribution: "A".into(),
            published_at: "2026-05-07T00:00:00Z".into(),
            tags: vec![],
            score: 0.5,
        }
    }

    #[test]
    fn trends_bundle_round_trip() {
        let bundle = TrendsBundle {
            schema_version: 1,
            generated_at: "2026-05-07T00:00:00Z".into(),
            expires_at: "2026-05-14T00:00:00Z".into(),
            curator_note_ko: "이번 주 핵심 흐름은 ...".into(),
            items: vec![sample_item()],
        };
        let s = serde_json::to_string(&bundle).unwrap();
        let parsed: TrendsBundle = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed.schema_version, 1);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].kind, TrendKind::Paper);
    }

    #[test]
    fn empty_bundle_round_trip() {
        let bundle = TrendsBundle {
            schema_version: 1,
            generated_at: "2026-05-07T00:00:00Z".into(),
            expires_at: "2026-05-14T00:00:00Z".into(),
            curator_note_ko: "".into(),
            items: vec![],
        };
        let s = serde_json::to_string(&bundle).unwrap();
        let parsed: TrendsBundle = serde_json::from_str(&s).unwrap();
        assert!(parsed.items.is_empty());
    }
}
