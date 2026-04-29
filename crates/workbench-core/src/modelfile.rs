//! Ollama Modelfile generator (FROM + PARAMETER + SYSTEM + 옵션 TEMPLATE).
//!
//! 정책 (phase-5p-workbench-decision.md §1.3):
//! - 한국어 system prompt: `\` → `\\`, `"` → `\"` escape. 줄바꿈은 보존 (triple-quoted block).
//! - PARAMETER stop은 여러 개 라인으로.
//! - TEMPLATE은 옵션. None이면 라인 자체 생략.
//! - multi-stage ADAPTER / MESSAGE는 v1.x 이월 (ADR-0023 §Decision 4).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelfileSpec {
    pub gguf_path: String,
    pub temperature: f32,
    pub num_ctx: u32,
    pub system_prompt_ko: String,
    pub stop_sequences: Vec<String>,
    pub template: Option<String>,
}

/// 따옴표 / 백슬래시 escape. 줄바꿈은 그대로 (triple-quoted block 사용).
pub fn escape_system_prompt(prompt: &str) -> String {
    prompt.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Modelfile 텍스트 렌더. Ollama Modelfile spec 기준.
pub fn render(spec: &ModelfileSpec) -> String {
    let mut out = String::new();
    out.push_str(&format!("FROM {}\n", spec.gguf_path));
    out.push_str(&format!("PARAMETER temperature {}\n", spec.temperature));
    out.push_str(&format!("PARAMETER num_ctx {}\n", spec.num_ctx));
    for stop in &spec.stop_sequences {
        out.push_str(&format!(
            "PARAMETER stop \"{}\"\n",
            escape_system_prompt(stop)
        ));
    }
    out.push_str("SYSTEM \"\"\"");
    out.push_str(&escape_system_prompt(&spec.system_prompt_ko));
    out.push_str("\"\"\"\n");
    if let Some(tmpl) = &spec.template {
        out.push_str("TEMPLATE \"\"\"");
        out.push_str(tmpl);
        out.push_str("\"\"\"\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn baseline_spec() -> ModelfileSpec {
        ModelfileSpec {
            gguf_path: "./models/test.gguf".into(),
            temperature: 0.7,
            num_ctx: 4096,
            system_prompt_ko: "한국어로 답해 주세요.".into(),
            stop_sequences: vec!["</s>".into()],
            template: None,
        }
    }

    #[test]
    fn basic_render_contains_required_directives() {
        let out = render(&baseline_spec());
        assert!(out.contains("FROM ./models/test.gguf"));
        assert!(out.contains("PARAMETER temperature 0.7"));
        assert!(out.contains("PARAMETER num_ctx 4096"));
        assert!(out.contains("PARAMETER stop \"</s>\""));
        assert!(out.contains("SYSTEM \"\"\"한국어로 답해 주세요.\"\"\""));
    }

    #[test]
    fn escape_quote() {
        let escaped = escape_system_prompt(r#"이건 "큰따옴표""#);
        assert_eq!(escaped, r#"이건 \"큰따옴표\""#);
    }

    #[test]
    fn escape_backslash() {
        let escaped = escape_system_prompt(r"a\b");
        assert_eq!(escaped, r"a\\b");
    }

    #[test]
    fn escape_combined_backslash_then_quote() {
        // 백슬래시 먼저 escape 되어야 quote escape의 backslash가 다시 escape되지 않음.
        let escaped = escape_system_prompt(r#"\"#);
        assert_eq!(escaped, r"\\");
        let escaped2 = escape_system_prompt(r#"\""#);
        assert_eq!(escaped2, r#"\\\""#);
    }

    #[test]
    fn multiline_korean_prompt_preserved() {
        let mut spec = baseline_spec();
        spec.system_prompt_ko = "첫 번째 줄.\n두 번째 줄.\n세 번째 줄.".into();
        let out = render(&spec);
        // 줄바꿈이 그대로 유지되어야 함 (triple-quoted block 안에서).
        assert!(out.contains("첫 번째 줄.\n두 번째 줄.\n세 번째 줄."));
    }

    #[test]
    fn multiple_stop_sequences() {
        let mut spec = baseline_spec();
        spec.stop_sequences = vec!["</s>".into(), "<|eot_id|>".into(), "<|im_end|>".into()];
        let out = render(&spec);
        assert!(out.contains("PARAMETER stop \"</s>\""));
        assert!(out.contains("PARAMETER stop \"<|eot_id|>\""));
        assert!(out.contains("PARAMETER stop \"<|im_end|>\""));
        // 세 라인이 모두 존재.
        let stop_lines = out
            .lines()
            .filter(|l| l.starts_with("PARAMETER stop"))
            .count();
        assert_eq!(stop_lines, 3);
    }

    #[test]
    fn template_optional_none_omits_line() {
        let spec = baseline_spec();
        let out = render(&spec);
        assert!(!out.contains("TEMPLATE"));
    }

    #[test]
    fn template_some_renders_block() {
        let mut spec = baseline_spec();
        spec.template = Some("{{ .Prompt }}".into());
        let out = render(&spec);
        assert!(out.contains("TEMPLATE \"\"\"{{ .Prompt }}\"\"\""));
    }

    #[test]
    fn temperature_exact_output() {
        let mut spec = baseline_spec();
        spec.temperature = 0.42;
        let out = render(&spec);
        assert!(out.contains("PARAMETER temperature 0.42"));
    }

    #[test]
    fn num_ctx_exact_output() {
        let mut spec = baseline_spec();
        spec.num_ctx = 8192;
        let out = render(&spec);
        assert!(out.contains("PARAMETER num_ctx 8192"));
    }

    #[test]
    fn system_prompt_with_quotes_escaped_in_render() {
        let mut spec = baseline_spec();
        spec.system_prompt_ko = r#"내 이름은 "AI"입니다."#.into();
        let out = render(&spec);
        // SYSTEM 블록 안의 큰따옴표가 escape 되어야 함.
        assert!(out.contains(r#"\"AI\""#));
    }

    #[test]
    fn stop_sequence_with_quote_is_escaped() {
        let mut spec = baseline_spec();
        spec.stop_sequences = vec![r#""end""#.into()];
        let out = render(&spec);
        // PARAMETER stop "..." 라인 안에서 quote escape.
        assert!(out.contains(r#"PARAMETER stop "\"end\""#));
    }

    #[test]
    fn render_starts_with_from_line() {
        let out = render(&baseline_spec());
        assert!(out.starts_with("FROM "));
    }
}
