//! SSE chunk parser — Phase 8'.c.4.
//!
//! 정책 (ADR-0030, phase-8p-9p-10p-residual-plan §2.8'.c.4):
//! - line-aware buffered parser. `\n\n` 분리자로 event를 추출.
//! - 한 chunk가 TCP segment 경계를 가로질러 도착해도 buffer에 누적 후 완전한 event 단위로 emit.
//! - `[DONE]` sentinel은 그대로 emit (caller가 그대로 forward).
//! - chunk JSON parse 실패 시 `ParseError`를 emit 하지만 caller는 원본을 그대로 forward (best-effort).
//! - 외부 의존성 추가 0 — bytes / serde_json만 사용.
//!
//! OpenAI streaming spec 핵심:
//! - 각 event는 `data: {json}\n\n` 또는 `data: [DONE]\n\n`.
//! - 빈 줄(`\n\n`)이 event 종결자.
//! - 다중 line event는 v1에서 미지원 (각 chunk는 단일 `data:` 줄).

use bytes::Bytes;
use thiserror::Error;

/// SSE chunk parse 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseChunk {
    /// `event:` 필드 값 (대부분 None — OpenAI는 사용 안 함).
    pub event: Option<String>,
    /// `data:` 필드 값 (JSON 또는 `[DONE]`).
    pub data: String,
    /// `id:` 필드 값 (대부분 None).
    pub id: Option<String>,
}

impl SseChunk {
    /// `[DONE]` sentinel인지.
    pub fn is_done(&self) -> bool {
        self.data.trim() == "[DONE]"
    }

    /// data를 JSON으로 파싱 시도. `[DONE]`이면 None.
    pub fn parse_json(&self) -> Option<Result<serde_json::Value, serde_json::Error>> {
        if self.is_done() {
            return None;
        }
        Some(serde_json::from_str(&self.data))
    }

    /// chunk를 SSE 와이어 포맷으로 직렬화 — `data: {data}\n\n` (+ event/id 옵션).
    ///
    /// 정책: byte-perfect 보장이 아니라 *valid SSE* 보장. 원본과 다를 수 있음(예: optional id 위치).
    /// PII redact 후 `reserialize` → 다시 stream에 push하기 위한 헬퍼.
    pub fn serialize(&self) -> String {
        let mut out = String::with_capacity(self.data.len() + 32);
        if let Some(ev) = &self.event {
            out.push_str("event: ");
            out.push_str(ev);
            out.push('\n');
        }
        if let Some(id) = &self.id {
            out.push_str("id: ");
            out.push_str(id);
            out.push('\n');
        }
        out.push_str("data: ");
        out.push_str(&self.data);
        out.push_str("\n\n");
        out
    }
}

#[derive(Debug, Error, Clone)]
pub enum ChunkError {
    /// invalid UTF-8.
    #[error("청크가 UTF-8이 아니에요: {0}")]
    InvalidUtf8(String),

    /// `data:` 필드 누락 (event-only frame).
    #[error("data 필드가 없는 SSE 이벤트예요")]
    DataMissing,
}

/// line-aware SSE buffer parser.
///
/// 사용 패턴:
/// ```ignore
/// let mut p = SseChunkParser::new();
/// while let Some(bytes) = upstream.next().await {
///     for result in p.parse(bytes) {
///         // ...
///     }
/// }
/// for result in p.flush() { /* 잔여 버퍼 처리 */ }
/// ```
pub struct SseChunkParser {
    /// 누적 buffer. `\n\n`를 만나기 전까지 보관.
    buf: String,
}

impl Default for SseChunkParser {
    fn default() -> Self {
        Self::new()
    }
}

impl SseChunkParser {
    pub fn new() -> Self {
        Self { buf: String::new() }
    }

    /// 새 bytes를 buffer에 추가하고 완성된 event를 추출.
    ///
    /// 반환: chunk 결과 vec. parse 실패한 frame도 `ChunkError`로 포함되어
    /// caller가 정책(원본 forward / drop / replace)을 결정할 수 있어요.
    pub fn parse(&mut self, bytes: Bytes) -> Vec<Result<SseChunk, ChunkError>> {
        // UTF-8 변환 — invalid면 lossy(replacement char)로 fallback. 단일 frame 단위에서
        // invalid byte가 섞이는 case는 거의 없어요.
        let s = match std::str::from_utf8(&bytes) {
            Ok(s) => s.to_string(),
            Err(_) => {
                let lossy = String::from_utf8_lossy(&bytes).into_owned();
                // 그래도 누적해 parse 시도 — invalid 문자가 있는 frame만 뒤에서 ParseError 발생.
                lossy
            }
        };
        self.buf.push_str(&s);
        self.drain_complete()
    }

    /// 완성된 frame만 buffer에서 빼서 결과 vec로 반환.
    ///
    /// 정책: `\n\n` 분리자를 기준으로 자른 뒤 마지막은 *불완전*일 수 있으니
    /// buffer에 그대로 남겨요. 다음 `parse()` 호출에서 이어 받음.
    fn drain_complete(&mut self) -> Vec<Result<SseChunk, ChunkError>> {
        let mut results = Vec::new();
        loop {
            // 정책: `\n\n` 분리자 검색. `\r\n\r\n`도 허용 (관용적).
            let sep_pos = self
                .buf
                .find("\n\n")
                .map(|p| (p, 2))
                .or_else(|| self.buf.find("\r\n\r\n").map(|p| (p, 4)));
            let (pos, sep_len) = match sep_pos {
                Some(v) => v,
                None => break, // 미완성 — 다음 chunk 대기.
            };
            let frame = self.buf[..pos].to_string();
            // pos + sep_len 이후가 다음 buffer.
            self.buf = self.buf[pos + sep_len..].to_string();
            // 빈 frame은 건너뜀 (`\n\n\n\n` 같은 케이스).
            if frame.trim().is_empty() {
                continue;
            }
            results.push(parse_event(&frame));
        }
        results
    }

    /// stream 종료 시 호출 — buffer에 남은 *완전한* 또는 *trailing* frame 처리.
    ///
    /// 정책: 남은 buffer가 비어있지 않으면 강제로 1 frame으로 간주하고 parse 시도.
    /// `[DONE]` sentinel이 separator 없이 도착하는 경우(드물지만 실제 발생) 대응.
    pub fn flush(&mut self) -> Vec<Result<SseChunk, ChunkError>> {
        let leftover = std::mem::take(&mut self.buf);
        if leftover.trim().is_empty() {
            return Vec::new();
        }
        vec![parse_event(&leftover)]
    }

    /// 현재 buffer가 비어있는지 (디버깅 / 테스트용).
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
}

/// 단일 frame (data: ... \n event: ... \n id: ...)을 파싱.
fn parse_event(frame: &str) -> Result<SseChunk, ChunkError> {
    let mut event: Option<String> = None;
    let mut data_lines: Vec<String> = Vec::new();
    let mut id: Option<String> = None;

    for line in frame.lines() {
        // SSE spec: comment line은 `:`로 시작.
        if line.starts_with(':') || line.is_empty() {
            continue;
        }
        // `data: foo` / `data:foo` 둘 다 허용. `:` 위치로 분리.
        let (field, value) = match line.find(':') {
            Some(idx) => {
                let f = &line[..idx];
                // 값 앞 단일 공백은 SSE spec에 따라 strip.
                let v = line[idx + 1..]
                    .strip_prefix(' ')
                    .unwrap_or(&line[idx + 1..]);
                (f, v)
            }
            None => (line, ""),
        };
        match field {
            "data" => data_lines.push(value.to_string()),
            "event" => event = Some(value.to_string()),
            "id" => id = Some(value.to_string()),
            _ => {
                // 알 수 없는 field — silently ignore (SSE spec).
            }
        }
    }
    if data_lines.is_empty() {
        return Err(ChunkError::DataMissing);
    }
    // 다중 data line은 `\n`으로 join (SSE spec).
    let data = data_lines.join("\n");
    Ok(SseChunk { event, data, id })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(s: &str) -> Bytes {
        Bytes::copy_from_slice(s.as_bytes())
    }

    // ── 정상 chunk ──────────────────────────────────────────────────────

    #[test]
    fn parses_single_data_chunk() {
        let mut p = SseChunkParser::new();
        let res = p.parse(b("data: {\"choices\":[]}\n\n"));
        assert_eq!(res.len(), 1);
        let chunk = res[0].as_ref().unwrap();
        assert_eq!(chunk.data, "{\"choices\":[]}");
        assert!(chunk.event.is_none());
        assert!(chunk.id.is_none());
    }

    #[test]
    fn parses_multiple_chunks_in_single_segment() {
        // 한 TCP segment에 chunk 3개.
        let mut p = SseChunkParser::new();
        let res = p.parse(b(
            "data: {\"a\":1}\n\ndata: {\"b\":2}\n\ndata: {\"c\":3}\n\n",
        ));
        assert_eq!(res.len(), 3);
        assert_eq!(res[0].as_ref().unwrap().data, "{\"a\":1}");
        assert_eq!(res[1].as_ref().unwrap().data, "{\"b\":2}");
        assert_eq!(res[2].as_ref().unwrap().data, "{\"c\":3}");
    }

    #[test]
    fn buffers_chunk_split_across_segments() {
        let mut p = SseChunkParser::new();
        let r1 = p.parse(b("data: {\"par"));
        // 미완성 — 결과 0.
        assert!(r1.is_empty());
        let r2 = p.parse(b("tial\":true}"));
        assert!(r2.is_empty());
        let r3 = p.parse(b("\n\n"));
        assert_eq!(r3.len(), 1);
        assert_eq!(r3[0].as_ref().unwrap().data, "{\"partial\":true}");
    }

    #[test]
    fn buffers_separator_split_across_segments() {
        // \n과 \n이 따로 도착.
        let mut p = SseChunkParser::new();
        let r1 = p.parse(b("data: x\n"));
        assert!(r1.is_empty());
        let r2 = p.parse(b("\n"));
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].as_ref().unwrap().data, "x");
    }

    // ── [DONE] sentinel ──────────────────────────────────────────────────

    #[test]
    fn done_sentinel_is_passed_through() {
        let mut p = SseChunkParser::new();
        let res = p.parse(b("data: [DONE]\n\n"));
        assert_eq!(res.len(), 1);
        let chunk = res[0].as_ref().unwrap();
        assert!(chunk.is_done());
        assert_eq!(chunk.data, "[DONE]");
        assert!(
            chunk.parse_json().is_none(),
            "is_done이면 parse_json은 None"
        );
    }

    #[test]
    fn done_after_data_chunks() {
        let mut p = SseChunkParser::new();
        let res = p.parse(b(
            "data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}]}\n\ndata: [DONE]\n\n",
        ));
        assert_eq!(res.len(), 2);
        assert!(!res[0].as_ref().unwrap().is_done());
        assert!(res[1].as_ref().unwrap().is_done());
    }

    // ── parse error ──────────────────────────────────────────────────────

    #[test]
    fn frame_without_data_returns_error() {
        // event:만 있는 frame은 SSE valid 하지만 OpenAI 안 씀 — 본 parser는 DataMissing.
        let mut p = SseChunkParser::new();
        let res = p.parse(b("event: ping\n\n"));
        assert_eq!(res.len(), 1);
        assert!(matches!(res[0], Err(ChunkError::DataMissing)));
    }

    #[test]
    fn invalid_json_data_still_returns_chunk() {
        // parser는 JSON parse 안 함 — chunk.data가 invalid JSON이어도 chunk 반환.
        let mut p = SseChunkParser::new();
        let res = p.parse(b("data: not-json-{at-all\n\n"));
        assert_eq!(res.len(), 1);
        let chunk = res[0].as_ref().unwrap();
        assert_eq!(chunk.data, "not-json-{at-all");
        let parsed = chunk.parse_json().unwrap();
        assert!(parsed.is_err(), "JSON parse는 실패해야");
    }

    // ── event / id 필드 ─────────────────────────────────────────────────

    #[test]
    fn parses_event_and_id_fields() {
        let mut p = SseChunkParser::new();
        let res = p.parse(b("event: message\nid: 42\ndata: {\"x\":1}\n\n"));
        assert_eq!(res.len(), 1);
        let chunk = res[0].as_ref().unwrap();
        assert_eq!(chunk.event.as_deref(), Some("message"));
        assert_eq!(chunk.id.as_deref(), Some("42"));
        assert_eq!(chunk.data, "{\"x\":1}");
    }

    // ── 다중 data line ──────────────────────────────────────────────────

    #[test]
    fn multi_line_data_is_joined_with_newline() {
        let mut p = SseChunkParser::new();
        let res = p.parse(b("data: line1\ndata: line2\n\n"));
        assert_eq!(res.len(), 1);
        let chunk = res[0].as_ref().unwrap();
        assert_eq!(chunk.data, "line1\nline2");
    }

    // ── CRLF separator ──────────────────────────────────────────────────

    #[test]
    fn crlf_separator_is_supported() {
        let mut p = SseChunkParser::new();
        let res = p.parse(b("data: {\"a\":1}\r\n\r\n"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].as_ref().unwrap().data, "{\"a\":1}");
    }

    // ── flush ───────────────────────────────────────────────────────────

    #[test]
    fn flush_drains_trailing_frame_without_separator() {
        let mut p = SseChunkParser::new();
        let res1 = p.parse(b("data: x\n"));
        assert!(res1.is_empty());
        let res2 = p.flush();
        assert_eq!(res2.len(), 1);
        assert_eq!(res2[0].as_ref().unwrap().data, "x");
    }

    #[test]
    fn flush_empty_buffer_is_noop() {
        let mut p = SseChunkParser::new();
        let res = p.flush();
        assert!(res.is_empty());
    }

    // ── serialize round-trip ────────────────────────────────────────────

    #[test]
    fn serialize_emits_valid_sse_frame() {
        let chunk = SseChunk {
            event: None,
            id: None,
            data: "{\"x\":1}".to_string(),
        };
        let serialized = chunk.serialize();
        assert_eq!(serialized, "data: {\"x\":1}\n\n");
        // round-trip.
        let mut p = SseChunkParser::new();
        let parsed = p.parse(b(&serialized));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].as_ref().unwrap().data, "{\"x\":1}");
    }

    #[test]
    fn serialize_includes_event_and_id() {
        let chunk = SseChunk {
            event: Some("msg".into()),
            id: Some("7".into()),
            data: "{}".into(),
        };
        let s = chunk.serialize();
        assert!(s.contains("event: msg\n"));
        assert!(s.contains("id: 7\n"));
        assert!(s.contains("data: {}\n\n"));
    }

    // ── empty / comment 처리 ────────────────────────────────────────────

    #[test]
    fn comment_lines_are_ignored() {
        let mut p = SseChunkParser::new();
        // `: keepalive\ndata: x\n\n` — 첫 line은 SSE spec의 comment.
        let res = p.parse(b(": keepalive\ndata: x\n\n"));
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].as_ref().unwrap().data, "x");
    }

    #[test]
    fn empty_frames_between_data_are_skipped() {
        let mut p = SseChunkParser::new();
        let res = p.parse(b("data: a\n\n\n\ndata: b\n\n"));
        assert_eq!(res.len(), 2);
        assert_eq!(res[0].as_ref().unwrap().data, "a");
        assert_eq!(res[1].as_ref().unwrap().data, "b");
    }

    // ── No-op pipeline 통과 시 byte-identical 보장 (caller가 reserialize 회피) ──

    /// caller가 *변경 없으면 원본 bytes를 그대로 forward*하는 정책에 부합하는지 확인.
    /// 본 테스트는 parse → 변경 없음 판단의 기반: chunk.data == 원본 substring.
    #[test]
    fn parsed_data_is_substring_of_original_when_no_transform() {
        let original = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\n";
        let mut p = SseChunkParser::new();
        let res = p.parse(b(original));
        let chunk = res[0].as_ref().unwrap();
        // 원본 frame 내 data 문자열이 그대로 보존돼요.
        assert!(original.contains(&chunk.data));
    }
}
