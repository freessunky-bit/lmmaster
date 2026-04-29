//! JSONL 자동 변환 — 4 포맷 (Alpaca / ShareGPT / OpenAI messages / 한국어 Q&A) → OpenAI messages 정규화.
//!
//! 정책 (phase-5p-workbench-decision.md §1.2):
//! - line 단위 자동 감지. 우선순위: messages > conversations > instruction+output > 질문+답변.
//! - 빈 line skip. 형식 오류 line은 `tracing::warn!` + skip (전체 파일 실패 회피).
//! - to_jsonl_line / write_jsonl로 정규화된 JSONL 출력.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::WorkbenchError;

/// 단일 메시지 (role + content). OpenAI chat schema와 동일.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// 정규화된 chat 예제 — `messages` 배열을 가진다.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatExample {
    pub messages: Vec<ChatMessage>,
}

/// 한 줄 JSON을 4 포맷 자동 감지 + 정규화. 우선순위:
/// 1. OpenAI `messages[]` (가장 명확)
/// 2. ShareGPT `conversations[]` (`from`/`value`)
/// 3. Alpaca `instruction` + (옵션 `input`) + `output`
/// 4. 한국어 Q&A `질문` + `답변`
pub fn parse_line(line: &str) -> Result<ChatExample, WorkbenchError> {
    let v: Value = serde_json::from_str(line)
        .map_err(|e| WorkbenchError::UnsupportedDataFormat(format!("JSON parse: {e}")))?;

    // 1. OpenAI messages — 가장 명확. role 정규화 (human → user, gpt/bot → assistant).
    if let Some(messages) = v.get("messages").and_then(|m| m.as_array()) {
        let parsed: Result<Vec<ChatMessage>, WorkbenchError> = messages
            .iter()
            .map(|m| {
                let role_raw = m.get("role").and_then(|r| r.as_str()).ok_or_else(|| {
                    WorkbenchError::UnsupportedDataFormat("messages[].role 누락".into())
                })?;
                let content = m.get("content").and_then(|c| c.as_str()).ok_or_else(|| {
                    WorkbenchError::UnsupportedDataFormat("messages[].content 누락".into())
                })?;
                let role = normalize_role(role_raw);
                Ok(ChatMessage {
                    role,
                    content: content.into(),
                })
            })
            .collect();
        return Ok(ChatExample { messages: parsed? });
    }

    // 2. ShareGPT — conversations 배열, from/value 키.
    if let Some(convs) = v.get("conversations").and_then(|c| c.as_array()) {
        let messages: Result<Vec<ChatMessage>, WorkbenchError> = convs
            .iter()
            .map(|c| {
                let from = c.get("from").and_then(|f| f.as_str()).ok_or_else(|| {
                    WorkbenchError::UnsupportedDataFormat("conversations[].from 누락".into())
                })?;
                let value = c.get("value").and_then(|v| v.as_str()).ok_or_else(|| {
                    WorkbenchError::UnsupportedDataFormat("conversations[].value 누락".into())
                })?;
                Ok(ChatMessage {
                    role: normalize_role(from),
                    content: value.into(),
                })
            })
            .collect();
        return Ok(ChatExample {
            messages: messages?,
        });
    }

    // 3. Alpaca — instruction + output (+ 옵션 input).
    if let (Some(instruction), Some(output)) = (
        v.get("instruction").and_then(|i| i.as_str()),
        v.get("output").and_then(|o| o.as_str()),
    ) {
        let input = v.get("input").and_then(|i| i.as_str()).unwrap_or("");
        let user_content = if input.is_empty() {
            instruction.to_string()
        } else {
            format!("{instruction}\n\n{input}")
        };
        return Ok(ChatExample {
            messages: vec![
                ChatMessage {
                    role: "user".into(),
                    content: user_content,
                },
                ChatMessage {
                    role: "assistant".into(),
                    content: output.into(),
                },
            ],
        });
    }

    // 4. 한국어 Q&A — "질문" + "답변".
    if let (Some(q), Some(a)) = (
        v.get("질문").and_then(|q| q.as_str()),
        v.get("답변").and_then(|a| a.as_str()),
    ) {
        return Ok(ChatExample {
            messages: vec![
                ChatMessage {
                    role: "user".into(),
                    content: q.into(),
                },
                ChatMessage {
                    role: "assistant".into(),
                    content: a.into(),
                },
            ],
        });
    }

    Err(WorkbenchError::UnsupportedDataFormat(
        "4 포맷(OpenAI/ShareGPT/Alpaca/한국어 Q&A) 중 어느 것에도 맞지 않아요".into(),
    ))
}

/// role 문자열을 OpenAI chat 4 enum에 정규화 (case-insensitive).
/// 알 수 없는 role은 그대로 보존 (예: tool, function).
fn normalize_role(raw: &str) -> String {
    match raw.to_ascii_lowercase().as_str() {
        "human" | "user" => "user".into(),
        "gpt" | "assistant" | "bot" | "ai" => "assistant".into(),
        "system" => "system".into(),
        "tool" | "function" => "tool".into(),
        _ => raw.to_string(),
    }
}

/// 전체 JSONL 텍스트 → 정규화된 examples. 빈 line / 잘못된 line은 skip + warn.
pub fn parse_jsonl(content: &str) -> Result<Vec<ChatExample>, WorkbenchError> {
    let mut out = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match parse_line(trimmed) {
            Ok(ex) => out.push(ex),
            Err(e) => tracing::warn!(line = idx + 1, error = %e, "JSONL line skipped"),
        }
    }
    Ok(out)
}

/// 정규화된 example 1건을 OpenAI messages JSONL line으로 직렬화.
pub fn to_jsonl_line(ex: &ChatExample) -> Result<String, WorkbenchError> {
    let v = serde_json::json!({ "messages": ex.messages });
    Ok(serde_json::to_string(&v)?)
}

/// 여러 examples를 JSONL 텍스트로. 마지막 line에도 newline.
pub fn write_jsonl(examples: &[ChatExample]) -> Result<String, WorkbenchError> {
    let mut out = String::new();
    for ex in examples {
        out.push_str(&to_jsonl_line(ex)?);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpaca_basic() {
        let line = r#"{"instruction":"한국의 수도는?","output":"서울입니다."}"#;
        let ex = parse_line(line).unwrap();
        assert_eq!(ex.messages.len(), 2);
        assert_eq!(ex.messages[0].role, "user");
        assert_eq!(ex.messages[0].content, "한국의 수도는?");
        assert_eq!(ex.messages[1].role, "assistant");
        assert_eq!(ex.messages[1].content, "서울입니다.");
    }

    #[test]
    fn alpaca_with_input() {
        let line = r#"{"instruction":"다음을 요약해 주세요.","input":"이 문장은 길어요.","output":"길어요."}"#;
        let ex = parse_line(line).unwrap();
        assert_eq!(ex.messages.len(), 2);
        assert_eq!(
            ex.messages[0].content,
            "다음을 요약해 주세요.\n\n이 문장은 길어요."
        );
        assert_eq!(ex.messages[1].content, "길어요.");
    }

    #[test]
    fn sharegpt_human_gpt_mapping() {
        let line = r#"{"conversations":[{"from":"human","value":"안녕"},{"from":"gpt","value":"안녕하세요"}]}"#;
        let ex = parse_line(line).unwrap();
        assert_eq!(ex.messages.len(), 2);
        assert_eq!(ex.messages[0].role, "user");
        assert_eq!(ex.messages[0].content, "안녕");
        assert_eq!(ex.messages[1].role, "assistant");
        assert_eq!(ex.messages[1].content, "안녕하세요");
    }

    #[test]
    fn sharegpt_with_system() {
        let line = r#"{"conversations":[{"from":"system","value":"너는 한국어 도우미야."},{"from":"human","value":"안녕"},{"from":"gpt","value":"안녕하세요"}]}"#;
        let ex = parse_line(line).unwrap();
        assert_eq!(ex.messages.len(), 3);
        assert_eq!(ex.messages[0].role, "system");
        assert_eq!(ex.messages[1].role, "user");
        assert_eq!(ex.messages[2].role, "assistant");
    }

    #[test]
    fn openai_messages_passthrough() {
        let line = r#"{"messages":[{"role":"user","content":"hi"},{"role":"assistant","content":"hello"}]}"#;
        let ex = parse_line(line).unwrap();
        assert_eq!(ex.messages.len(), 2);
        assert_eq!(ex.messages[0].role, "user");
        assert_eq!(ex.messages[1].role, "assistant");
    }

    #[test]
    fn openai_messages_role_normalization() {
        // role이 "human" / "gpt"이면 → user / assistant.
        let line =
            r#"{"messages":[{"role":"human","content":"hi"},{"role":"gpt","content":"hello"}]}"#;
        let ex = parse_line(line).unwrap();
        assert_eq!(ex.messages[0].role, "user");
        assert_eq!(ex.messages[1].role, "assistant");
    }

    #[test]
    fn korean_qa_qja() {
        let line = r#"{"질문":"한국의 수도는?","답변":"서울"}"#;
        let ex = parse_line(line).unwrap();
        assert_eq!(ex.messages.len(), 2);
        assert_eq!(ex.messages[0].role, "user");
        assert_eq!(ex.messages[0].content, "한국의 수도는?");
        assert_eq!(ex.messages[1].role, "assistant");
        assert_eq!(ex.messages[1].content, "서울");
    }

    #[test]
    fn mixed_format_jsonl_all_normalized() {
        let content = "\
{\"instruction\":\"a\",\"output\":\"A\"}
{\"conversations\":[{\"from\":\"human\",\"value\":\"b\"},{\"from\":\"gpt\",\"value\":\"B\"}]}
{\"messages\":[{\"role\":\"user\",\"content\":\"c\"},{\"role\":\"assistant\",\"content\":\"C\"}]}
{\"질문\":\"d\",\"답변\":\"D\"}
";
        let examples = parse_jsonl(content).unwrap();
        assert_eq!(examples.len(), 4);
        for ex in &examples {
            assert!(!ex.messages.is_empty());
            assert!(ex.messages.iter().any(|m| m.role == "user"));
            assert!(ex.messages.iter().any(|m| m.role == "assistant"));
        }
    }

    #[test]
    fn malformed_line_skipped() {
        let content = "\
{\"instruction\":\"a\",\"output\":\"A\"}
not even json
{\"질문\":\"b\",\"답변\":\"B\"}
";
        let examples = parse_jsonl(content).unwrap();
        assert_eq!(examples.len(), 2);
    }

    #[test]
    fn empty_line_skipped() {
        let content = "\
{\"instruction\":\"a\",\"output\":\"A\"}


{\"질문\":\"b\",\"답변\":\"B\"}
";
        let examples = parse_jsonl(content).unwrap();
        assert_eq!(examples.len(), 2);
    }

    #[test]
    fn unknown_format_returns_error() {
        let line = r#"{"foo":"bar"}"#;
        let err = parse_line(line).unwrap_err();
        match err {
            WorkbenchError::UnsupportedDataFormat(msg) => {
                assert!(msg.contains("4 포맷"));
            }
            other => panic!("expected UnsupportedDataFormat, got {other:?}"),
        }
    }

    #[test]
    fn to_jsonl_line_round_trip() {
        let ex = ChatExample {
            messages: vec![
                ChatMessage {
                    role: "user".into(),
                    content: "안녕".into(),
                },
                ChatMessage {
                    role: "assistant".into(),
                    content: "반가워요".into(),
                },
            ],
        };
        let line = to_jsonl_line(&ex).unwrap();
        let parsed = parse_line(&line).unwrap();
        assert_eq!(parsed, ex);
    }

    #[test]
    fn write_jsonl_round_trip() {
        let examples = vec![
            ChatExample {
                messages: vec![
                    ChatMessage {
                        role: "user".into(),
                        content: "a".into(),
                    },
                    ChatMessage {
                        role: "assistant".into(),
                        content: "A".into(),
                    },
                ],
            },
            ChatExample {
                messages: vec![
                    ChatMessage {
                        role: "user".into(),
                        content: "b".into(),
                    },
                    ChatMessage {
                        role: "assistant".into(),
                        content: "B".into(),
                    },
                ],
            },
        ];
        let text = write_jsonl(&examples).unwrap();
        assert!(text.ends_with('\n'));
        let parsed = parse_jsonl(&text).unwrap();
        assert_eq!(parsed, examples);
    }

    #[test]
    fn role_unknown_preserved_in_messages() {
        // tool role은 그대로 보존.
        let line = r#"{"messages":[{"role":"tool","content":"output"}]}"#;
        let ex = parse_line(line).unwrap();
        assert_eq!(ex.messages[0].role, "tool");
    }

    #[test]
    fn sharegpt_unknown_role_preserved() {
        // ShareGPT의 unknown role은 그대로 (사용자 정의).
        let line = r#"{"conversations":[{"from":"narrator","value":"옛날옛날에"}]}"#;
        let ex = parse_line(line).unwrap();
        assert_eq!(ex.messages[0].role, "narrator");
    }

    #[test]
    fn alpaca_missing_output_falls_through_to_error() {
        // instruction만 있고 output 없으면 unknown.
        let line = r#"{"instruction":"a","input":"b"}"#;
        let err = parse_line(line).unwrap_err();
        assert!(matches!(err, WorkbenchError::UnsupportedDataFormat(_)));
    }

    #[test]
    fn messages_missing_content_returns_error() {
        let line = r#"{"messages":[{"role":"user"}]}"#;
        let err = parse_line(line).unwrap_err();
        match err {
            WorkbenchError::UnsupportedDataFormat(msg) => {
                assert!(msg.contains("content"));
            }
            other => panic!("expected UnsupportedDataFormat, got {other:?}"),
        }
    }
}
