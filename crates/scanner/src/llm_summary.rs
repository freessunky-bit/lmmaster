//! Ollama HTTP API 클라이언트 + 모델 cascade + 응답 검증.
//!
//! 정책 (ADR-0020):
//! - `POST /api/generate` `stream: false`, `keep_alive: "30s"`.
//! - 모델 cascade: EXAONE → HCX-SEED → Qwen2.5/Llama3.2.
//! - `GET /api/tags` 결과 1h 캐시.
//! - 응답 한국어 검증 (hangul ≥30%, <800 chars, no chat template leak).
//! - 검증 실패 시 `LlmValidationFailed` → caller가 deterministic fallback.
//! - **자동 모델 pull 금지** (v1).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::checks::CheckResult;
use crate::error::ScannerError;

const CASCADE_TTL: Duration = Duration::from_secs(60 * 60);

/// 기본 cascade — ADR-0020 §4 우선순위.
pub const DEFAULT_CASCADE: &[&str] = &[
    "exaone3.5:2.4b",
    "exaone:1.2b",
    "exaone:7.8b",
    "hyperclova-x-seed-text-instruct:8b",
    "qwen2.5:3b",
    "llama3.2:3b",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaTagsResponse {
    pub models: Vec<OllamaModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModel {
    pub name: String,
}

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    keep_alive: &'a str,
    options: GenerateOptions,
}

#[derive(Debug, Serialize)]
struct GenerateOptions {
    temperature: f32,
    num_predict: u32,
    top_p: f32,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
    #[allow(dead_code)]
    #[serde(default)]
    done: bool,
}

/// LLM 클라이언트. cascade 캐시를 mutex로 보호.
pub struct OllamaClient {
    http: reqwest::Client,
    endpoint: String,
    cascade: Vec<String>,
    cache: Mutex<CascadeCache>,
}

struct CascadeCache {
    chosen: Option<String>,
    refreshed_at: Option<Instant>,
}

impl OllamaClient {
    pub fn new(endpoint: impl Into<String>, cascade: Vec<String>) -> Result<Self, ScannerError> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(60))
            .pool_idle_timeout(Duration::from_secs(30))
            .no_proxy()
            .build()?;
        Ok(Self {
            http,
            endpoint: endpoint.into(),
            cascade,
            cache: Mutex::new(CascadeCache {
                chosen: None,
                refreshed_at: None,
            }),
        })
    }

    /// cascade 중 설치된 첫 모델 이름 반환. 1h 캐시.
    pub async fn pick_model(&self) -> Result<String, ScannerError> {
        // 1h 내 캐시면 재사용.
        {
            let c = self.cache.lock().expect("cascade cache poisoned");
            if let (Some(chosen), Some(at)) = (&c.chosen, c.refreshed_at) {
                if at.elapsed() < CASCADE_TTL {
                    return Ok(chosen.clone());
                }
            }
        }

        // /api/tags 호출.
        let url = format!("{}/api/tags", self.endpoint.trim_end_matches('/'));
        let resp = match self.http.get(&url).send().await {
            Ok(r) => r,
            Err(e) if e.is_connect() => return Err(ScannerError::OllamaUnreachable),
            Err(e) if e.is_timeout() => return Err(ScannerError::OllamaTimeout),
            Err(e) => return Err(ScannerError::Http(e)),
        };
        if !resp.status().is_success() {
            return Err(ScannerError::OllamaUnreachable);
        }
        let tags: OllamaTagsResponse = resp.json().await?;
        let installed: Vec<String> = tags.models.iter().map(|m| m.name.clone()).collect();

        // cascade prefix match.
        let mut chosen: Option<String> = None;
        for needle in &self.cascade {
            if let Some(found) = installed.iter().find(|n| name_matches(n, needle)) {
                chosen = Some(found.clone());
                break;
            }
        }
        let chosen = chosen.ok_or(ScannerError::OllamaModelMissing)?;

        // 캐시 갱신.
        let mut c = self.cache.lock().expect("cascade cache poisoned");
        c.chosen = Some(chosen.clone());
        c.refreshed_at = Some(Instant::now());
        Ok(chosen)
    }

    /// 점검 결과를 한국어 요약으로 변환. 실패 시 caller가 deterministic fallback 사용.
    pub async fn summarize(
        &self,
        env_summary: &str,
        checks: &[CheckResult],
    ) -> Result<String, ScannerError> {
        let model = self.pick_model().await?;
        let prompt = build_prompt(env_summary, checks);

        let body = OllamaGenerateRequest {
            model: &model,
            prompt: &prompt,
            stream: false,
            keep_alive: "30s",
            options: GenerateOptions {
                temperature: 0.4,
                num_predict: 400,
                top_p: 0.9,
            },
        };

        let url = format!("{}/api/generate", self.endpoint.trim_end_matches('/'));
        let resp = match self.http.post(&url).json(&body).send().await {
            Ok(r) => r,
            Err(e) if e.is_connect() => return Err(ScannerError::OllamaUnreachable),
            Err(e) if e.is_timeout() => return Err(ScannerError::OllamaTimeout),
            Err(e) => return Err(ScannerError::Http(e)),
        };
        if !resp.status().is_success() {
            return Err(ScannerError::OllamaUnreachable);
        }
        let parsed: OllamaGenerateResponse = resp.json().await?;
        let text = validate_korean_summary(&parsed.response)?;
        Ok(text)
    }
}

fn name_matches(installed: &str, needle: &str) -> bool {
    // exact match 우선, prefix match (정확한 cascade entry는 quant suffix가 빠질 수 있음).
    if installed == needle {
        return true;
    }
    installed.starts_with(needle)
}

fn build_prompt(env_summary: &str, checks: &[CheckResult]) -> String {
    let system = "너는 LMmaster라는 한국어 데스크톱 AI 도우미의 진단 요약 도우미야. \
                  사용자에게 점검 결과를 따뜻하고 짧게 한국어 해요체로 정리해줘. \
                  판단(권장/권장 안 함)은 하지 말고, 사실만 풀어 말해줘. \
                  답은 4~5 문장 평문으로만 작성해줘.";
    let issues_json = serde_json::to_string(checks).unwrap_or_else(|_| "[]".to_string());
    format!("{system}\n\n환경: {env_summary}\n점검 결과(JSON): {issues_json}\n\n한국어 요약:")
}

/// 응답 검증 — 한국어 비율 + 길이 + 템플릿 누수 체크.
pub fn validate_korean_summary(s: &str) -> Result<String, ScannerError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(ScannerError::LlmValidationFailed("empty"));
    }
    if s.chars().count() > 800 {
        return Err(ScannerError::LlmValidationFailed("too long"));
    }
    let total = s.chars().count();
    let hangul = s
        .chars()
        .filter(|c| ('\u{AC00}'..='\u{D7AF}').contains(c))
        .count();
    if (hangul as f32) / (total as f32) < 0.30 {
        return Err(ScannerError::LlmValidationFailed("not korean"));
    }
    for tok in [
        "<|im_start|>",
        "<|im_end|>",
        "[INST]",
        "[/INST]",
        "</s>",
        "<|endoftext|>",
        "<|begin_of_text|>",
    ] {
        if s.contains(tok) {
            return Err(ScannerError::LlmValidationFailed("template leak"));
        }
    }
    Ok(s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_matches_exact_and_prefix() {
        assert!(name_matches("exaone:7.8b", "exaone:7.8b"));
        assert!(name_matches("exaone:7.8b-q4_K_M", "exaone:7.8b"));
        assert!(!name_matches("exaone:1.2b", "exaone:7.8b"));
        assert!(!name_matches("llama3.2:3b", "qwen2.5:3b"));
    }

    #[test]
    fn validate_rejects_empty_string() {
        assert!(matches!(
            validate_korean_summary(""),
            Err(ScannerError::LlmValidationFailed("empty"))
        ));
        assert!(matches!(
            validate_korean_summary("   "),
            Err(ScannerError::LlmValidationFailed("empty"))
        ));
    }

    #[test]
    fn validate_accepts_korean_text() {
        let ok = validate_korean_summary("안녕하세요. 점검 결과는 모두 정상이에요.");
        assert!(ok.is_ok());
    }

    #[test]
    fn validate_rejects_english_only() {
        let r = validate_korean_summary("Everything looks fine. No issues detected.");
        assert!(matches!(
            r,
            Err(ScannerError::LlmValidationFailed("not korean"))
        ));
    }

    #[test]
    fn validate_rejects_template_leak() {
        // 한국어 비중 30% 통과시켜 leak 검증 분기까지 도달하도록.
        let r = validate_korean_summary(
            "안녕하세요 점검 결과는 정상이에요 모두 잘 동작해요 <|im_end|>",
        );
        assert!(matches!(
            r,
            Err(ScannerError::LlmValidationFailed("template leak"))
        ));
    }

    #[test]
    fn validate_rejects_too_long() {
        let s = "가".repeat(801);
        assert!(matches!(
            validate_korean_summary(&s),
            Err(ScannerError::LlmValidationFailed("too long"))
        ));
    }
}
