//! Pipeline schema — Phase 23'.c.2 (ADR-0063 §4).
//!
//! 정책:
//! - `DatasetIngestStage` 5단계 + Done — 각 단계별 진행 이벤트 emit.
//! - `IngestProgress` — Tauri channel + UI 진행 다이얼로그.
//! - `SampleStrategy` — full / stratified-{N} / first-{N}.
//! - 본 sub-phase (23'.c.2.a)는 schema 정의만. 실 흐름은 23'.c.2.c (pipeline runner).

use serde::{Deserialize, Serialize};

/// Dataset import 5단계 + Done — UI 진행 다이얼로그 매핑.
///
/// 각 단계 의미:
/// - `Manifest`: HF `/api/datasets/{ds}/parquet/{config}/{split}` 응답 받기.
/// - `Downloading`: row group 단위 parquet streaming (Range request).
/// - `Chunking`: text-splitter로 row narrative → chunks (512/64 default).
/// - `Embedding`: OnnxEmbedder cascade (KURE-v1 / bge-m3 / multilingual-e5).
/// - `Writing`: SQLCipher datasets + chunks 테이블 INSERT.
/// - `Done`: 완료. UI toast + Workspace > 지식 자료에 카드 자동 등장.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DatasetIngestStage {
    Manifest,
    Downloading,
    Chunking,
    Embedding,
    Writing,
    Done,
}

/// 진행 이벤트 — Tauri `emit_to(window, "dataset-import:progress", IngestProgress)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestProgress {
    pub stage: DatasetIngestStage,
    /// 현재 진행 단위 (rows / chunks / bytes) — stage별 의미 다름.
    pub current: u64,
    /// 총 예상 단위. 0 = unknown (예: streaming 시작 직후).
    pub total: u64,
    /// 추정 남은 시간 (초). None = 계산 미가능 (시작 직후).
    pub eta_secs: Option<u64>,
    /// 누적 chunk 수 (UI 카드 메타 업데이트용).
    pub chunks_written: u64,
    /// 사람 향 한국어 해요체 메시지 — UI hint chip.
    pub message_ko: String,
}

impl IngestProgress {
    pub fn new(stage: DatasetIngestStage, message_ko: impl Into<String>) -> Self {
        Self {
            stage,
            current: 0,
            total: 0,
            eta_secs: None,
            chunks_written: 0,
            message_ko: message_ko.into(),
        }
    }
}

/// 샘플링 전략 — 사용자가 import 시 슬라이더로 선택.
///
/// - `Full`: 전체 row import. 100만+ row면 경고 modal 후만.
/// - `Stratified { n, by }`: 컬럼 균등 분포 (예: `province × occupation` 100인).
/// - `First { n }`: 처음 N row만 (빠른 미리보기).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SampleStrategy {
    Full,
    Stratified { n: u64, by: Vec<String> },
    First { n: u64 },
}

impl SampleStrategy {
    /// 권장 default — 10K stratified by province × occupation (Personas-Korea 기준).
    pub fn default_recommended() -> Self {
        Self::Stratified {
            n: 10_000,
            by: vec!["province".into(), "occupation".into()],
        }
    }

    /// 사용자에게 표시할 한국어 라벨.
    pub fn label_ko(&self) -> String {
        match self {
            Self::Full => "전체 (경고: 100만 row 시 GPU 30분/CPU 5시간)".into(),
            Self::Stratified { n, by } => {
                format!("{} 명 균등 분포 ({})", n, by.join(" × "))
            }
            Self::First { n } => format!("처음 {} 행 (미리보기)", n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_stage_round_trip_kebab() {
        for (stage, expected) in [
            (DatasetIngestStage::Manifest, "manifest"),
            (DatasetIngestStage::Downloading, "downloading"),
            (DatasetIngestStage::Chunking, "chunking"),
            (DatasetIngestStage::Embedding, "embedding"),
            (DatasetIngestStage::Writing, "writing"),
            (DatasetIngestStage::Done, "done"),
        ] {
            let v = serde_json::to_value(stage).unwrap();
            assert_eq!(v.as_str(), Some(expected));
            let parsed: DatasetIngestStage = serde_json::from_value(v).unwrap();
            assert_eq!(parsed, stage);
        }
    }

    #[test]
    fn ingest_progress_initial() {
        let p = IngestProgress::new(DatasetIngestStage::Manifest, "데이터셋 정보 받고 있어요");
        assert_eq!(p.stage, DatasetIngestStage::Manifest);
        assert_eq!(p.current, 0);
        assert_eq!(p.total, 0);
        assert!(p.eta_secs.is_none());
        assert!(p.message_ko.contains("받고"));
    }

    #[test]
    fn sample_strategy_default_is_10k_stratified() {
        let s = SampleStrategy::default_recommended();
        match s {
            SampleStrategy::Stratified { n, by } => {
                assert_eq!(n, 10_000);
                assert_eq!(by, vec!["province", "occupation"]);
            }
            _ => panic!("default should be Stratified"),
        }
    }

    #[test]
    fn sample_strategy_labels_korean() {
        assert!(SampleStrategy::Full.label_ko().contains("전체"));
        assert!(SampleStrategy::default_recommended()
            .label_ko()
            .contains("10000"));
        assert!(SampleStrategy::First { n: 100 }.label_ko().contains("100"));
    }

    #[test]
    fn sample_strategy_round_trip_tagged() {
        let s = SampleStrategy::Stratified {
            n: 1000,
            by: vec!["province".into()],
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["kind"], "stratified");
        assert_eq!(v["n"], 1000);
        let parsed: SampleStrategy = serde_json::from_value(v).unwrap();
        assert_eq!(parsed, s);
    }
}
