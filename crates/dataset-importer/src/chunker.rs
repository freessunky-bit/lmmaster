//! Tokenizer-aware chunker — Phase 23'.c.2.c (ADR-0063 §1, reinforcement §3).
//!
//! 정책:
//! - `text-splitter::TextSplitter` + tokenizers feature로 RecursiveCharacterTextSplitter Rust 포팅.
//! - tokenizers 크레이트로 *KURE-v1 / multilingual-e5와 동일* 토크나이저 사용 (production 경로).
//! - 토크나이저 미주입 시 *char-based fallback* (한국어 평균 1.5~2 char/token으로 보수 추정).
//! - chunk_size = 512 tokens / overlap = 64 (12.5%) — RAG 표준 + KURE-v1 native context.
//! - deterministic — text-splitter 자체가 결정적.
//!
//! Provider trait dispatch — `TextSplitter<S: ChunkSizer>`의 generic을 erase해 enum 분기 없이
//! `Tokenizer` / `Characters` 두 sizer를 동일 box에 담는다. .c.2.d (Service runner)에서 동일 chunker
//! 인스턴스를 row마다 재사용 → splitter 1회 생성으로 cost amortize.

#![allow(dead_code)]

use text_splitter::{ChunkConfig, ChunkSizer, TextSplitter};
use tokenizers::Tokenizer;

use crate::error::{DatasetImportError, DatasetImportResult};

/// Type-erased chunk provider — `TextSplitter<S>`를 trait object로 감싸 generic param 제거.
trait ChunkProvider: Send + Sync {
    fn chunks(&self, text: &str) -> Vec<String>;
}

impl<S> ChunkProvider for TextSplitter<S>
where
    S: ChunkSizer + Send + Sync + 'static,
{
    fn chunks(&self, text: &str) -> Vec<String> {
        TextSplitter::chunks(self, text)
            .map(str::to_string)
            .collect()
    }
}

/// Chunk 설정 — chunk_size / overlap.
#[derive(Debug, Clone, Copy)]
pub struct ChunkConfigParams {
    /// 최대 토큰 수 (KURE-v1 / multilingual-e5 native = 512).
    pub max_tokens: usize,
    /// 오버랩 (10~25% 권장, 12.5% = 64 default).
    pub overlap: usize,
}

impl ChunkConfigParams {
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

/// Dataset chunker — text-splitter recursive splitter wrapper.
///
/// `with_tokenizer`로 production-grade tokenizer 주입 (KURE-v1 / multilingual-e5),
/// `with_char_fallback`으로 토크나이저 없이도 동작 (첫 실행, 모델 미설치 케이스).
pub struct DatasetChunker {
    provider: Box<dyn ChunkProvider>,
}

impl DatasetChunker {
    /// Tokenizer 기반 splitter — 정확한 token count.
    pub fn with_tokenizer(
        tokenizer: Tokenizer,
        params: ChunkConfigParams,
    ) -> DatasetImportResult<Self> {
        params.validate()?;
        let config = ChunkConfig::new(params.max_tokens)
            .with_overlap(params.overlap)
            .map_err(|e| DatasetImportError::ChunkingFailed(e.to_string()))?
            .with_sizer(tokenizer);
        Ok(Self {
            provider: Box::new(TextSplitter::new(config)),
        })
    }

    /// Char-based fallback — 토크나이저 없을 때.
    /// 한국어 평균 1.5~2 char/token으로 max_tokens × 2 보수 추정.
    pub fn with_char_fallback(params: ChunkConfigParams) -> DatasetImportResult<Self> {
        params.validate()?;
        let char_size = params.max_tokens.saturating_mul(2);
        let char_overlap = params.overlap.saturating_mul(2);
        let config = ChunkConfig::new(char_size)
            .with_overlap(char_overlap)
            .map_err(|e| DatasetImportError::ChunkingFailed(e.to_string()))?;
        Ok(Self {
            provider: Box::new(TextSplitter::new(config)),
        })
    }

    /// row 텍스트 → chunks. text 비었으면 빈 vec.
    pub fn chunks(&self, row_index: u64, text: &str) -> Vec<DatasetChunk> {
        if text.is_empty() {
            return Vec::new();
        }
        self.provider
            .chunks(text)
            .into_iter()
            .enumerate()
            .map(|(i, c)| DatasetChunk {
                row_index,
                chunk_index: i as u32,
                text: c,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default_kure_v1() {
        let c = ChunkConfigParams::default_kure_v1();
        assert_eq!(c.max_tokens, 512);
        assert_eq!(c.overlap, 64);
        assert!(c.validate().is_ok());
    }

    #[test]
    fn config_invalid_overlap_rejected() {
        let c = ChunkConfigParams {
            max_tokens: 100,
            overlap: 100,
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn config_zero_tokens_rejected() {
        let c = ChunkConfigParams {
            max_tokens: 0,
            overlap: 0,
        };
        assert!(c.validate().is_err());
    }

    #[test]
    fn chunker_short_text_one_chunk() {
        let c = DatasetChunker::with_char_fallback(ChunkConfigParams::default_kure_v1()).unwrap();
        let chunks = c.chunks(0, "짧은 텍스트입니다.");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].row_index, 0);
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[0].text, "짧은 텍스트입니다.");
    }

    #[test]
    fn chunker_empty_text_no_chunks() {
        let c = DatasetChunker::with_char_fallback(ChunkConfigParams::default_kure_v1()).unwrap();
        let chunks = c.chunks(0, "");
        assert_eq!(chunks.len(), 0);
    }

    #[test]
    fn chunker_long_text_multiple_chunks() {
        // 작은 chunk_size로 분할 강제.
        let c = DatasetChunker::with_char_fallback(ChunkConfigParams {
            max_tokens: 30,
            overlap: 5,
        })
        .unwrap();
        // 한국어 narrative 반복 — text-splitter 권장 split point (sentence + word) 자연 발생.
        let text =
            "안녕하세요. 가상 한국인입니다. 페르소나 narrative. 시뮬레이션 실험. 토큰 분할 검증."
                .repeat(20);
        let chunks = c.chunks(42, &text);
        assert!(chunks.len() > 1, "long text should yield multiple chunks");
        assert_eq!(chunks[0].row_index, 42);
        assert_eq!(chunks[0].chunk_index, 0);
        // chunk_index 단조 증가.
        for w in chunks.windows(2) {
            assert!(w[1].chunk_index > w[0].chunk_index);
        }
    }

    #[test]
    fn chunker_deterministic_100x() {
        let c = DatasetChunker::with_char_fallback(ChunkConfigParams::default_kure_v1()).unwrap();
        let text = "한국어 페르소나 narrative 테스트. ".repeat(100);
        let first = c.chunks(0, &text);
        for _ in 0..100 {
            let next = c.chunks(0, &text);
            assert_eq!(first, next, "chunker must be deterministic");
        }
    }

    #[test]
    fn chunker_korean_utf8_safe() {
        let c = DatasetChunker::with_char_fallback(ChunkConfigParams::default_kure_v1()).unwrap();
        let text = "안녕하세요 LMmaster입니다. 가상 한국인 100인을 시뮬레이션해요.";
        let chunks = c.chunks(0, text);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.text.chars().count() > 0);
        }
    }

    /// Tokenizer 주입 경로 — production wiring 검증 (실 KURE-v1 tokenizer.json 없이도
    /// 최소 WordLevel + dummy vocab으로 trait 호환만 확인).
    #[test]
    fn chunker_with_tokenizer_constructs() {
        use tokenizers::models::wordlevel::WordLevelBuilder;

        let mut vocab = std::collections::HashMap::new();
        vocab.insert("[UNK]".to_string(), 0);
        vocab.insert("hello".to_string(), 1);
        vocab.insert("world".to_string(), 2);
        let model = WordLevelBuilder::new()
            .vocab(vocab)
            .unk_token("[UNK]".into())
            .build()
            .expect("WordLevel build");

        let tokenizer = Tokenizer::new(model);

        let chunker =
            DatasetChunker::with_tokenizer(tokenizer, ChunkConfigParams::default_kure_v1())
                .expect("with_tokenizer");
        let chunks = chunker.chunks(0, "hello world hello world");
        assert!(!chunks.is_empty());
    }
}
