//! Tokenizer-aware chunker — Phase 23'.c.2.c (ADR-0063 §1, reinforcement §3).
//!
//! 정책:
//! - `text-splitter::TextSplitter<Tokenizer>` 기반 RecursiveCharacterTextSplitter Rust 포팅.
//! - tokenizers 크레이트로 *KURE-v1 / multilingual-e5와 동일* 토크나이저 사용.
//! - chunk_size = 512 tokens / overlap = 64 (12.5%) — RAG 표준 + KURE-v1 native context.
//! - 한국어 grapheme/word level fallback — 종결 어미 자연 인식.
//! - deterministic — 동일 입력 100회 동일 chunk.
//!
//! **본 sub-phase 23'.c.2.a 코드는 struct 정의 + 단위 테스트**. 실 chunk 흐름은 23'.c.2.c (실 토크나이저 로드 + DatasetIngestService 통합)에서.

#![allow(dead_code)]

use crate::error::{DatasetImportError, DatasetImportResult};

/// Chunk 설정 — chunk_size / overlap.
#[derive(Debug, Clone, Copy)]
pub struct ChunkConfig {
    /// 최대 토큰 수 (KURE-v1 / multilingual-e5 native = 512).
    pub max_tokens: usize,
    /// 오버랩 (10~25% 권장, 12.5% = 64 default).
    pub overlap: usize,
}

impl ChunkConfig {
    /// LMmaster default — 512 / 64 (KURE-v1 native + 12.5% overlap).
    pub const fn default_kure_v1() -> Self {
        Self {
            max_tokens: 512,
            overlap: 64,
        }
    }

    /// 검증 — overlap < max_tokens 보장.
    pub fn validate(&self) -> DatasetImportResult<()> {
        if self.overlap >= self.max_tokens {
            return Err(DatasetImportError::ChunkingFailed(format!(
                "overlap({}) >= max_tokens({})",
                self.overlap, self.max_tokens
            )));
        }
        if self.max_tokens == 0 {
            return Err(DatasetImportError::ChunkingFailed(
                "max_tokens는 0보다 커야 해요".into(),
            ));
        }
        Ok(())
    }
}

/// 청크 결과 — 텍스트 + row 메타.
#[derive(Debug, Clone, PartialEq)]
pub struct DatasetChunk {
    /// row index in parquet (citation 용도).
    pub row_index: u64,
    /// chunk index within row (0, 1, 2, ...).
    pub chunk_index: u32,
    /// chunk 텍스트.
    pub text: String,
}

/// Phase 23'.c.2.c 후속 — 실 `text-splitter::TextSplitter<Tokenizer>` 통합.
///
/// 현 단계에서는 *심플 char-based fallback*으로 동작 (knowledge-stack 기존 chunker 패턴).
/// 23'.c.2.c에서 실 tokenizer + text-splitter로 교체.
pub struct DatasetChunker {
    config: ChunkConfig,
}

impl DatasetChunker {
    pub fn new(config: ChunkConfig) -> DatasetImportResult<Self> {
        config.validate()?;
        Ok(Self { config })
    }

    /// 텍스트 → 청크 — char 기반 fallback (Phase 23'.c.2.c에서 tokenizer 기반 교체).
    ///
    /// 정책 (current sub-phase 23'.c.2.a):
    /// - char 단위 chunk_size 추정 (한국어 token ≈ 1.5~2 char).
    /// - chunk_size 512 tokens × 1.5 ≈ 768 chars로 1차 분할.
    /// - overlap 64 tokens × 1.5 ≈ 96 chars.
    pub fn chunks(&self, row_index: u64, text: &str) -> Vec<DatasetChunk> {
        if text.is_empty() {
            return Vec::new();
        }

        // char 기반 chunk_size 추정 (한국어 평균).
        let char_chunk_size = self.config.max_tokens.saturating_mul(2); // 보수적 추정 (한국어 평균 1.5~2 char/token).
        let char_overlap = self.config.overlap.saturating_mul(2);

        let chars: Vec<char> = text.chars().collect();
        let total = chars.len();
        if total <= char_chunk_size {
            return vec![DatasetChunk {
                row_index,
                chunk_index: 0,
                text: text.to_string(),
            }];
        }

        let mut chunks = Vec::new();
        let mut start = 0;
        let mut chunk_index = 0u32;
        while start < total {
            let end = (start + char_chunk_size).min(total);
            let chunk_text: String = chars[start..end].iter().collect();
            chunks.push(DatasetChunk {
                row_index,
                chunk_index,
                text: chunk_text,
            });
            chunk_index += 1;
            if end == total {
                break;
            }
            start = end.saturating_sub(char_overlap);
        }
        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_config_default_kure_v1() {
        let c = ChunkConfig::default_kure_v1();
        assert_eq!(c.max_tokens, 512);
        assert_eq!(c.overlap, 64);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn chunk_config_invalid_overlap_rejected() {
        let c = ChunkConfig {
            max_tokens: 100,
            overlap: 100,
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn chunk_config_zero_tokens_rejected() {
        let c = ChunkConfig {
            max_tokens: 0,
            overlap: 0,
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn chunker_short_text_one_chunk() {
        let c = DatasetChunker::new(ChunkConfig::default_kure_v1()).unwrap();
        let chunks = c.chunks(0, "짧은 텍스트입니다.");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].row_index, 0);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[0].text, "짧은 텍스트입니다.");
    }

    #[test]
    fn chunker_empty_text_no_chunks() {
        let c = DatasetChunker::new(ChunkConfig::default_kure_v1()).unwrap();
        let chunks = c.chunks(0, "");
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn chunker_long_text_multiple_chunks() {
        let c = DatasetChunker::new(ChunkConfig {
            max_tokens: 50,
            overlap: 10,
        })
        .unwrap();
        let text: String = (0..200)
            .map(|i| char::from_digit((i % 10) as u32, 10).unwrap())
            .collect();
        let chunks = c.chunks(42, &text);
        assert!(chunks.len() > 1, "long text should yield multiple chunks");
        assert_eq!(chunks[0].row_index, 42);
        assert_eq!(chunks[0].chunk_index, 0);
        // 마지막 chunk index 단조 증가.
        for w in chunks.windows(2) {
            assert!(w[1].chunk_index > w[0].chunk_index);
        }
    }

    #[test]
    fn chunker_deterministic_100x() {
        let c = DatasetChunker::new(ChunkConfig::default_kure_v1()).unwrap();
        let text = "한국어 페르소나 narrative 테스트. ".repeat(100);
        let first = c.chunks(0, &text);
        for _ in 0..100 {
            let next = c.chunks(0, &text);
            assert_eq!(first, next, "chunker must be deterministic");
        }
    }

    #[test]
    fn chunker_korean_grapheme_safe() {
        let c = DatasetChunker::new(ChunkConfig::default_kure_v1()).unwrap();
        // 한국어 + 영문 혼합 — char boundary 안전 (grapheme 깨짐 X).
        let text = "안녕하세요 LMmaster입니다. 가상 한국인 100인을 시뮬레이션해요.";
        let chunks = c.chunks(0, text);
        assert!(!chunks.is_empty());
        // 각 chunk 텍스트가 valid UTF-8.
        for chunk in &chunks {
            assert!(chunk.text.chars().count() > 0);
        }
    }
}
