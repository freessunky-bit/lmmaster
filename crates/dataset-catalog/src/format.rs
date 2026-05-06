//! 데이터셋 포맷 — Phase 23'.a.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DatasetFormat {
    Parquet,
    Jsonl,
    Csv,
    Arrow,
    Tsv,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ChunkStrategy {
    /// 재귀 문자 분할 — LangChain RecursiveCharacterTextSplitter 패턴.
    RecursiveCharacter,
    /// 문장 경계 분할.
    Sentence,
    /// 단순 토큰 N개씩.
    Token,
    /// row 단위 (parquet/jsonl 1 row = 1 chunk).
    Row,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_format_round_trip() {
        for (fmt, expected) in [
            (DatasetFormat::Parquet, "parquet"),
            (DatasetFormat::Jsonl, "jsonl"),
            (DatasetFormat::Csv, "csv"),
            (DatasetFormat::Arrow, "arrow"),
            (DatasetFormat::Tsv, "tsv"),
        ] {
            let v = serde_json::to_value(fmt).unwrap();
            assert_eq!(v.as_str(), Some(expected));
            let parsed: DatasetFormat = serde_json::from_value(v).unwrap();
            assert_eq!(parsed, fmt);
        }
    }

    #[test]
    fn chunk_strategy_round_trip() {
        for (strat, expected) in [
            (ChunkStrategy::RecursiveCharacter, "recursive-character"),
            (ChunkStrategy::Sentence, "sentence"),
            (ChunkStrategy::Token, "token"),
            (ChunkStrategy::Row, "row"),
        ] {
            let v = serde_json::to_value(strat).unwrap();
            assert_eq!(v.as_str(), Some(expected));
            let parsed: ChunkStrategy = serde_json::from_value(v).unwrap();
            assert_eq!(parsed, strat);
        }
    }
}
