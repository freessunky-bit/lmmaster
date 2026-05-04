//! HuggingFace Hub Search — Phase 11'.c (ADR-0049).
//!
//! 정책:
//! - 외부 통신 0 정책 예외 (ADR-0026 §1) — `huggingface.co` 화이트리스트 기존 (hf_meta.rs와 같은 갈래).
//! - HF Hub Search API: `GET https://huggingface.co/api/models?search={q}&limit=20&sort=likes`.
//! - rate limit unauth 1000 req/h — 사용자 검색 빈도 충분.
//! - 검색 결과는 노란 "지원 외" 라벨 — 큐레이션 thesis(ADR-0049 §A 거부) 보존.
//! - 빈 query → 빈 결과 (네트워크 호출 X).
//! - 한국어 graceful 에러.

use std::sync::Arc;
use std::time::Duration;

use model_registry::{CustomModel, ModelRegistry as CustomModelRegistry};
use serde::{Deserialize, Serialize};
use tauri::State;

const SEARCH_LIMIT: usize = 20;
const SEARCH_TIMEOUT_SEC: u64 = 8;

/// HF Hub Search API 응답의 일부 — 우리가 쓰는 필드만.
#[derive(Debug, Deserialize)]
struct HfApiSearchHit {
    /// repo 형식 — `org/name` 또는 `name`.
    id: String,
    #[serde(default)]
    downloads: Option<u64>,
    #[serde(default)]
    likes: Option<u64>,
    #[serde(default, rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default)]
    pipeline_tag: Option<String>,
    #[serde(default)]
    library_name: Option<String>,
    #[serde(default)]
    private: Option<bool>,
}

/// UI에 노출할 검색 hit — repo + 메타.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HfSearchHit {
    /// `org/name` HF repo 경로 (사용자에게 그대로 표시).
    pub repo: String,
    pub downloads: u64,
    pub likes: u64,
    /// RFC3339 — 빈 문자열 가능.
    pub last_modified: String,
    /// HF pipeline_tag (예: "text-generation"). UI 카드 보조 라벨.
    pub pipeline_tag: Option<String>,
    /// HF library_name (예: "transformers", "gguf"). GGUF 호환성 hint.
    pub library_name: Option<String>,
}

/// 한국어 graceful 에러.
#[derive(Debug, thiserror::Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum HfSearchError {
    #[error("HuggingFace 응답을 받지 못했어요. 잠시 뒤에 다시 시도해 주세요.")]
    Network { message: String },
    #[error("HuggingFace 검색 결과를 해석하지 못했어요.")]
    Parse { message: String },
    #[error("HuggingFace 서버 오류 ({status}). 잠시 뒤에 다시 시도해 주세요.")]
    Upstream { status: u16 },
}

/// HF Hub Search API 호출. 빈 query는 즉시 빈 결과 반환 (네트워크 호출 X).
pub async fn search_models(
    http: &reqwest::Client,
    query: &str,
) -> Result<Vec<HfSearchHit>, HfSearchError> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let url = format!(
        "https://huggingface.co/api/models?search={}&limit={}&sort=likes",
        urlencoding::encode(trimmed),
        SEARCH_LIMIT,
    );

    let resp = http
        .get(&url)
        .timeout(Duration::from_secs(SEARCH_TIMEOUT_SEC))
        .send()
        .await
        .map_err(|e| HfSearchError::Network {
            message: e.to_string(),
        })?;

    let status = resp.status();
    if !status.is_success() {
        return Err(HfSearchError::Upstream {
            status: status.as_u16(),
        });
    }

    let body: Vec<HfApiSearchHit> = resp.json().await.map_err(|e| HfSearchError::Parse {
        message: e.to_string(),
    })?;

    let hits = body
        .into_iter()
        .filter(|h| !matches!(h.private, Some(true))) // private repo 노출 X.
        .map(|h| HfSearchHit {
            repo: h.id,
            downloads: h.downloads.unwrap_or(0),
            likes: h.likes.unwrap_or(0),
            last_modified: h.last_modified.unwrap_or_default(),
            pipeline_tag: h.pipeline_tag,
            library_name: h.library_name,
        })
        .collect();

    Ok(hits)
}

/// 사용자 검색 IPC. 매 호출마다 새 reqwest client (검색 빈도가 낮아 비용 미미).
///
/// 한국어 graceful 에러 (HfSearchError 한국어 메시지). 외부 통신 0 정책 예외 — ADR-0026 §1.
#[tauri::command]
pub async fn search_hf_models(query: String) -> Result<Vec<HfSearchHit>, HfSearchError> {
    // Phase R-C (ADR-0055) — .no_proxy() 강제 + 폴백 제거. HF Search API는 huggingface.co 화이트리스트만.
    let http = reqwest::Client::builder()
        .no_proxy()
        .user_agent("LMmaster-desktop")
        .timeout(Duration::from_secs(SEARCH_TIMEOUT_SEC))
        .build()
        .expect("reqwest Client builder must succeed (TLS init)");
    search_models(&http, &query).await
}

/// 사용자가 HF 검색 결과 모델을 "지금 시도해 볼게요" 클릭 시 — 사용자 PC의 CustomModelRegistry에 등록.
///
/// 정책 (ADR-0049):
/// - 큐레이션 외 모델 = `notes` warning 자동 prepend.
/// - artifact_paths 빈 vec / eval=0/0 / lora_adapter=None — HF repo 직접 풀.
/// - id는 `hf-{repo}` (slash → dash) — 카탈로그 entries id 충돌 회피.
#[tauri::command]
pub async fn register_hf_model(
    model_registry: State<'_, Arc<CustomModelRegistry>>,
    repo: String,
    file: Option<String>,
) -> Result<CustomModel, String> {
    let id = format!("hf-{}", repo.replace('/', "-"));
    let modelfile = format!(
        "# 큐레이션되지 않은 모델이에요 — chat template / quantization을 사용자가 검증해 주세요.\nFROM hf.co/{repo}{}\n",
        file.as_ref()
            .map(|f| format!(":{f}"))
            .unwrap_or_default()
    );
    let custom = CustomModel {
        id: id.clone(),
        base_model: format!("hf.co/{repo}"),
        quant_type: "auto".into(),
        lora_adapter: None,
        modelfile,
        created_at: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        eval_passed: 0,
        eval_total: 0,
        artifact_paths: Vec::new(),
    };
    model_registry
        .register(custom.clone())
        .map_err(|e| e.to_string())?;
    Ok(custom)
}

/// repo URL → "큐레이션 추가 요청" GitHub Issue prefilled URL.
///
/// 사용자가 이 URL을 시스템 브라우저로 열면 GitHub Issue 작성 화면에 repo가 미리 채워짐.
/// `.github/ISSUE_TEMPLATE/curation-request.yml`이 form 정의 + URL 쿼리 매개변수로 prefill.
pub fn curation_request_url(repo: &str) -> String {
    let title = format!("[큐레이션 요청] {repo}");
    let body = format!(
        "## 모델 정보\n\n- HuggingFace repo: https://huggingface.co/{repo}\n\n## 사용 의도\n\n(어떤 작업에 쓰고 싶은지 적어주세요 — 예: \"한국어 코딩\", \"비전 + 한국어\")\n\n## 추가 메모\n\n(선택)\n",
    );
    format!(
        "https://github.com/freessunky-bit/lmmaster/issues/new?template=curation-request.yml&title={}&body={}",
        urlencoding::encode(&title),
        urlencoding::encode(&body),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_query_returns_empty_without_network() {
        // network 없이도 빈 결과 보장 (단위 테스트가 외부 호출 X).
        let http = reqwest::Client::new();
        assert!(search_models(&http, "").await.unwrap().is_empty());
        assert!(search_models(&http, "   ").await.unwrap().is_empty());
    }

    #[test]
    fn search_error_serializes_with_kebab_kind() {
        let e = HfSearchError::Network {
            message: "down".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "network");

        let e = HfSearchError::Upstream { status: 503 };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "upstream");
        assert_eq!(v["status"], 503);
    }

    #[test]
    fn search_hit_round_trip() {
        let h = HfSearchHit {
            repo: "elyza/Llama-3-ELYZA-JP-8B".into(),
            downloads: 12345,
            likes: 247,
            last_modified: "2026-04-20T12:00:00Z".into(),
            pipeline_tag: Some("text-generation".into()),
            library_name: Some("transformers".into()),
        };
        let s = serde_json::to_string(&h).unwrap();
        let h2: HfSearchHit = serde_json::from_str(&s).unwrap();
        assert_eq!(h2, h);
    }

    #[test]
    fn search_error_messages_are_korean() {
        let e = HfSearchError::Network {
            message: "down".into(),
        };
        assert!(e.to_string().contains("HuggingFace"));
        assert!(e.to_string().contains("받지 못했"));

        let e = HfSearchError::Upstream { status: 500 };
        assert!(e.to_string().contains("서버 오류"));
        assert!(e.to_string().contains("500"));
    }

    #[test]
    fn curation_request_url_includes_repo_and_template() {
        let url = curation_request_url("elyza/Llama-3-ELYZA-JP-8B");
        assert!(url.starts_with("https://github.com/"));
        assert!(url.contains("template=curation-request.yml"));
        // urlencoded title — kebab-case가 아닌 percent-encoded.
        assert!(url.contains("title=") && url.contains("body="));
    }
}
