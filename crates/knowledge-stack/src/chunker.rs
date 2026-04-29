//! Korean-aware chunker.
//!
//! 파이프라인 (ADR-0024 §2):
//!   1. NFC 정규화 (`unicode-normalization`).
//!   2. 단락 분할 (`\n\n`).
//!   3. 단락이 target_size 초과 시 문장 분할 (`. ! ?` + 한국어 종결 어미 `다` `까` 뒤 공백).
//!   4. 그래도 초과 시 char-window fallback (multi-byte 한글 중간 절단 방지 + overlap).
//!
//! 주의: target_size / overlap 단위는 *문자 수*. byte 단위가 아니라 char 단위로 다뤄
//! 한국어 char-boundary 안전성을 자연스럽게 확보.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

/// 정규화된 chunk 1개. id는 content prefix sha256 hex 16자.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chunk {
    pub id: String,
    pub content: String,
    /// chunk가 시작되는 char index (정규화된 입력 기준).
    pub start: usize,
    /// chunk가 끝나는 char index (exclusive).
    pub end: usize,
}

/// NFC 정규화 + 공백 normalize. 외부 노출 (caller가 query 시 동일 변환 적용 필요).
pub fn normalize_korean(text: &str) -> String {
    let nfc: String = text.nfc().collect();
    // 다중 공백·탭·\r은 단일 공백으로. paragraph break(\n\n)는 보존.
    let mut out = String::with_capacity(nfc.len());
    let mut consecutive_newlines = 0u8;
    let mut paragraph_break_emitted = false;
    for ch in nfc.chars() {
        if ch == '\n' {
            consecutive_newlines = consecutive_newlines.saturating_add(1);
            if consecutive_newlines >= 2 && !paragraph_break_emitted {
                // 직전에 single \n으로 push된 공백이 있으면 제거하고 paragraph break 추가.
                if out.ends_with(' ') {
                    out.pop();
                }
                out.push('\n');
                out.push('\n');
                paragraph_break_emitted = true;
            } else if consecutive_newlines == 1 && !out.ends_with(' ') && !out.ends_with('\n') {
                out.push(' ');
            }
        } else if ch.is_whitespace() {
            // \r / \t / 일반 공백 — 단일 공백으로 collapse.
            consecutive_newlines = 0;
            paragraph_break_emitted = false;
            if !out.ends_with(' ') && !out.ends_with('\n') {
                out.push(' ');
            }
        } else {
            out.push(ch);
            consecutive_newlines = 0;
            paragraph_break_emitted = false;
        }
    }
    out.trim().to_string()
}

fn chunk_id(content: &str) -> String {
    let mut hasher = Sha256::new();
    // 처음 256 char만 hashing — 매우 긴 chunk라도 prefix가 같으면 동일 id.
    let prefix: String = content.chars().take(256).collect();
    hasher.update(prefix.as_bytes());
    let digest = hasher.finalize();
    hex::encode(&digest[..8])
}

/// 한국어 친화 chunker — NFC → 단락 → 문장 → 글자 윈도 fallback.
///
/// `target_size` / `overlap`은 *문자 수* 기준.
/// `target_size = 0`이면 빈 Vec 반환 (방어).
pub fn chunk_text(input: &str, target_size: usize, overlap: usize) -> Vec<Chunk> {
    if input.trim().is_empty() || target_size == 0 {
        return Vec::new();
    }
    let normalized = normalize_korean(input);
    if normalized.is_empty() {
        return Vec::new();
    }

    // 1단계: 단락 분할.
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut cursor: usize = 0; // char-index cursor in `normalized`.
    let total_chars = normalized.chars().count();
    let paragraphs: Vec<&str> = normalized.split("\n\n").collect();

    for para in paragraphs {
        let para_chars = para.chars().count();
        // cursor를 단락 시작점에 정렬.
        let para_start = find_char_index(&normalized, para, cursor).unwrap_or(cursor);
        let para_end = para_start + para_chars;

        if para_chars == 0 {
            cursor = para_end + 2; // "\n\n"
            continue;
        }

        if para_chars <= target_size {
            // 그대로 chunk 1개.
            push_chunk(&mut chunks, para, para_start, para_end);
        } else {
            // 2단계: 문장 분할.
            let sentences = split_sentences(para);
            let mut sentence_chunks = group_sentences(&sentences, target_size, overlap);
            // 만약 문장 단위가 너무 커서 여전히 target을 한참 넘기면 char-window fallback.
            let mut overflowed: Vec<Chunk> = Vec::new();
            for sc in sentence_chunks.drain(..) {
                if sc.content.chars().count() > target_size + overlap.max(1) {
                    let (cw, _) = char_window(&sc.content, target_size, overlap, sc.start);
                    overflowed.extend(cw);
                } else {
                    overflowed.push(sc);
                }
            }
            // sentence_chunks는 단락 내부 offset 기반이므로 para_start 더해 반영.
            for mut c in overflowed {
                c.start += para_start;
                c.end += para_start;
                if c.end > para_end {
                    c.end = para_end;
                }
                chunks.push(c);
            }
        }
        // paragraph 종료 후 cursor 이동.
        cursor = para_end + 2;
        if cursor > total_chars {
            cursor = total_chars;
        }
    }

    chunks
}

fn push_chunk(chunks: &mut Vec<Chunk>, content: &str, start: usize, end: usize) {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return;
    }
    chunks.push(Chunk {
        id: chunk_id(trimmed),
        content: trimmed.to_string(),
        start,
        end,
    });
}

/// `text` 안에서 `needle` 첫 등장 char-index. cursor 이상에서.
fn find_char_index(text: &str, needle: &str, from: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(from);
    }
    // char-index 기반 검색 — UTF-8 byte index와 다르게 안전.
    let chars: Vec<char> = text.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    if needle_chars.is_empty() {
        return Some(from);
    }
    if from > chars.len() {
        return None;
    }
    let limit = chars.len().saturating_sub(needle_chars.len());
    for i in from..=limit {
        if chars[i..i + needle_chars.len()] == needle_chars[..] {
            return Some(i);
        }
    }
    None
}

/// 문장 분할 — `. ! ?` 또는 한국어 종결 어미 `다` `까` 뒤 공백/줄바꿈.
fn split_sentences(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut buf = String::new();
    let chars: Vec<char> = text.chars().collect();
    for i in 0..chars.len() {
        let ch = chars[i];
        buf.push(ch);
        let next = chars.get(i + 1).copied();
        let is_terminal = matches!(ch, '.' | '!' | '?')
            || (matches!(ch, '다' | '까') && next.map(|c| c.is_whitespace()).unwrap_or(true));
        if is_terminal
            && (next.map(|c| c.is_whitespace()).unwrap_or(true)
                || ch == '.'
                || ch == '!'
                || ch == '?')
        {
            // `다` / `까`는 다음 문자가 공백이거나 끝일 때만 boundary — 단어 안의 `까` 분리 방지.
            if matches!(ch, '다' | '까') && !next.map(|c| c.is_whitespace()).unwrap_or(true) {
                continue;
            }
            let s = buf.trim().to_string();
            if !s.is_empty() {
                out.push(s);
            }
            buf.clear();
        }
    }
    let last = buf.trim();
    if !last.is_empty() {
        out.push(last.to_string());
    }
    if out.is_empty() && !text.trim().is_empty() {
        out.push(text.trim().to_string());
    }
    out
}

/// 문장 list를 target_size 안쪽으로 묶어 chunk Vec로. paragraph-local offset (start=0 기준).
fn group_sentences(sentences: &[String], target_size: usize, overlap: usize) -> Vec<Chunk> {
    let mut chunks: Vec<Chunk> = Vec::new();
    let mut buf = String::new();
    let mut buf_start: usize = 0;
    let mut cursor: usize = 0;

    for s in sentences {
        let s_chars = s.chars().count();
        // 현재 buf + 다음 문장이 target을 넘기면 flush.
        let buf_chars = buf.chars().count();
        if buf_chars + s_chars + 1 > target_size && buf_chars > 0 {
            let end = buf_start + buf_chars;
            push_chunk(&mut chunks, &buf, buf_start, end);
            // overlap만큼 buf의 tail을 다음 chunk의 head로.
            let tail_chars: usize = overlap.min(buf_chars);
            let tail: String = buf.chars().skip(buf_chars - tail_chars).collect();
            buf = tail;
            buf_start = end - tail_chars;
        }
        if !buf.is_empty() && !buf.ends_with(' ') {
            buf.push(' ');
            cursor = cursor.saturating_add(1);
        }
        if buf.is_empty() {
            buf_start = cursor;
        }
        buf.push_str(s);
        cursor += s_chars;
    }
    if !buf.trim().is_empty() {
        let end = buf_start + buf.chars().count();
        push_chunk(&mut chunks, &buf, buf_start, end);
    }
    chunks
}

/// 글자 단위 윈도 — multi-byte 한글 중간 절단 방지 + overlap.
/// 반환: (chunks, 마지막 cursor).
fn char_window(
    text: &str,
    target_size: usize,
    overlap: usize,
    base_start: usize,
) -> (Vec<Chunk>, usize) {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Chunk> = Vec::new();
    let mut start = 0usize;
    let total = chars.len();
    let stride = target_size.saturating_sub(overlap).max(1);
    while start < total {
        let end = (start + target_size).min(total);
        let slice: String = chars[start..end].iter().collect();
        push_chunk(&mut out, &slice, base_start + start, base_start + end);
        if end >= total {
            break;
        }
        start += stride;
    }
    (out, base_start + total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_nfc_idempotent() {
        // NFD 자모 분리 vs NFC 완성형 — 정규화 결과가 동일해야 함.
        let nfd = "\u{1100}\u{1161}\u{11A8}"; // ㄱ + ㅏ + ㄱ
        let nfc = "각";
        assert_eq!(normalize_korean(nfd), normalize_korean(nfc));
    }

    #[test]
    fn empty_input_returns_empty() {
        assert!(chunk_text("", 100, 20).is_empty());
        assert!(chunk_text("   \t  \n", 100, 20).is_empty());
    }

    #[test]
    fn single_short_paragraph_one_chunk() {
        let chunks = chunk_text("안녕하세요. 반갑습니다.", 100, 10);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("안녕"));
    }

    #[test]
    fn paragraph_split_on_double_newline() {
        let text = "첫 번째 단락이에요.\n\n두 번째 단락이에요.";
        let chunks = chunk_text(text, 200, 20);
        assert_eq!(chunks.len(), 2);
        assert!(chunks[0].content.contains("첫 번째"));
        assert!(chunks[1].content.contains("두 번째"));
    }

    #[test]
    fn long_paragraph_falls_to_sentence_split() {
        // target=30 → 단락 1개가 30자 초과 → 문장 분할 진입.
        let text = "이것은 첫 문장이에요. 이것은 두 번째 문장이에요. 이것은 세 번째 문장이에요. 이것은 네 번째 문장이에요.";
        let chunks = chunk_text(text, 30, 5);
        assert!(chunks.len() >= 2);
        // 모든 chunk content가 valid (NFC) — char_indices가 깨지지 않음.
        for c in &chunks {
            assert!(!c.content.is_empty());
        }
    }

    #[test]
    fn sentence_split_korean_endings() {
        // "다" + 공백을 boundary로 인식.
        let para: String = "한국어 예제를 처리합니다 한국어 예제를 처리합니다 한국어 예제를 처리합니다 한국어 예제를 처리합니다".into();
        let sentences = split_sentences(&para);
        // "다 " 마다 끊김.
        assert!(
            sentences.len() >= 2,
            "expected ≥2 sentences, got {sentences:?}"
        );
    }

    #[test]
    fn char_window_no_byte_split_in_korean() {
        // target_size 5 char로 잘라도 multi-byte 한글이 깨지면 안 됨.
        let text = "한국어".repeat(20); // 60 chars, 180 bytes.
        let chunks = chunk_text(&text, 5, 1);
        assert!(!chunks.is_empty());
        for c in &chunks {
            // valid UTF-8 (push_chunk는 trim된 String을 저장).
            assert!(!c.content.is_empty());
            // chars()로 walk할 수 있어야 함.
            let count = c.content.chars().count();
            assert!(count > 0);
        }
    }

    #[test]
    fn overlap_correctness_overlap_present() {
        // 충분히 긴 문장 1개 — char-window fallback 진입.
        let text: String = "abcdefghij".repeat(20); // 200 chars ASCII.
        let chunks = chunk_text(&text, 30, 10);
        assert!(chunks.len() >= 2);
        // 인접 chunk 사이의 overlap 검증 — 다음 chunk start - 이전 chunk end는 stride로 음수 (겹침).
        for window in chunks.windows(2) {
            let prev_end = window[0].end;
            let next_start = window[1].start;
            assert!(
                next_start <= prev_end,
                "overlap expected: prev_end={prev_end} next_start={next_start}"
            );
        }
    }

    #[test]
    fn content_10x_target_size_produces_many_chunks() {
        let text: String = "안녕하세요 ".repeat(500); // 약 3000 chars.
        let chunks = chunk_text(&text, 100, 20);
        assert!(
            chunks.len() >= 5,
            "expected ≥5 chunks, got {}",
            chunks.len()
        );
    }

    #[test]
    fn deterministic_ids() {
        let text = "동일한 텍스트는 동일한 id를 가져야 해요.";
        let a = chunk_text(text, 200, 20);
        let b = chunk_text(text, 200, 20);
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.id, y.id);
            assert_eq!(x.content, y.content);
        }
    }

    #[test]
    fn single_line_no_terminal_punctuation() {
        let chunks = chunk_text("문장부호 없는 문장 하나만 있어요", 100, 10);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].content.contains("문장부호"));
    }

    #[test]
    fn normalize_collapses_consecutive_whitespace() {
        let n = normalize_korean("a   b\t\tc");
        assert_eq!(n, "a b c");
    }

    #[test]
    fn normalize_preserves_paragraph_break() {
        let n = normalize_korean("a\n\nb");
        assert!(n.contains("\n\n"));
    }
}
