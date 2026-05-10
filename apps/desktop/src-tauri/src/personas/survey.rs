//! 설문 배치 실행 — N명 페르소나 × M개 문항을 로컬 LLM에 순차 호출.
//!
//! 정책:
//! - frontend가 personas[] + survey 정의를 보내면 backend가 OllamaAdapter / LlamaCppAdapter로 N×M 호출.
//! - 각 호출 결과는 PersonasSurveyEvent::Answer로 stream → frontend 실시간 표시.
//! - cancel token으로 중단 가능 (창 닫힘 / 명시 cancel).
//! - 한 페르소나가 한 번에 모든 질문에 답하도록 system prompt에 페르소나 narrative + 모든 질문 묶음
//!   대신, 페르소나 1명 × 질문 1개 단위로 호출 → 안정성·재시도 단위 작게.

use std::sync::{Arc, Mutex};

use adapter_ollama::OllamaAdapter;
use chat_protocol::{ChatEvent, ChatMessage, ChatOutcome};
use serde::{Deserialize, Serialize};
use shared_types::RuntimeKind;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use super::sample::Persona;
use crate::chat::llama_cpp::{
    build_server_spec, ensure_model_files_present, LlamaServerState, ManagedLlamaServer,
};
use crate::chat::registry::ChatRegistry;
use crate::commands::CatalogState;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PersonasSurveyError {
    #[error("내부 오류: {message}")]
    Internal { message: String },
    #[error("지원하지 않는 런타임이에요: {runtime}")]
    UnsupportedRuntime { runtime: String },
    #[error("모델이 로드되지 않았어요: {message}")]
    ModelNotReady { message: String },
}

/// 설문 1문항.
#[derive(Debug, Clone, Deserialize)]
pub struct SurveyQuestion {
    pub id: String,
    /// "single" | "multi" | "scale" | "open"
    #[serde(rename = "type")]
    pub q_type: String,
    pub text: String,
    /// 객관식 보기 — single/multi에서만.
    #[serde(default)]
    pub options: Vec<String>,
    /// 척도 설명 — scale에서만 (예: "1=전혀 안 씀, 5=거의 매일").
    #[serde(default)]
    pub scale: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SurveyDef {
    pub survey_id: String,
    pub title: String,
    pub questions: Vec<SurveyQuestion>,
}

/// 한 페르소나의 한 문항 응답.
#[derive(Debug, Clone, Serialize)]
pub struct SurveyAnswer {
    pub persona_uuid: String,
    pub question_id: String,
    pub answer: String,
    pub took_ms: u64,
}

/// 진행 이벤트 — Channel<PersonasSurveyEvent>로 frontend stream.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PersonasSurveyEvent {
    Started { total_calls: usize },
    Progress {
        completed: usize,
        total: usize,
        current_persona: String,
        current_question: String,
    },
    Answer { answer: SurveyAnswer },
    Completed { count: usize, total_ms: u64 },
    Cancelled,
    Failed { message: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunSurveyArgs {
    pub personas: Vec<Persona>,
    pub survey: SurveyDef,
    pub runtime_kind: RuntimeKind,
    pub model_id: String,
    /// 추가 system 지시문 (사용자 정의). 페르소나 narrative 앞에 붙음.
    #[serde(default)]
    pub system_extra: Option<String>,
}

fn build_system_prompt(p: &Persona, extra: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(x) = extra {
        if !x.is_empty() {
            out.push_str(x);
            out.push_str("\n\n");
        }
    }
    out.push_str("당신은 다음 인구통계와 배경을 가진 한국인이에요. 이 사람의 입장에서, 평소 말투로, 한국어로 답해 주세요.\n\n");
    out.push_str(&format!("성별: {}\n", p.sex));
    out.push_str(&format!("나이: {}\n", p.age));
    if !p.province.is_empty() {
        out.push_str(&format!("거주지: {}\n", p.province));
    }
    if !p.occupation.is_empty() {
        out.push_str(&format!("직업: {}\n", p.occupation));
    }
    if !p.persona.is_empty() {
        out.push_str(&format!("배경: {}\n", p.persona));
    }
    out
}

fn build_user_prompt(q: &SurveyQuestion) -> String {
    let mut out = String::new();
    out.push_str(&q.text);
    out.push('\n');
    match q.q_type.as_str() {
        "single" => {
            if !q.options.is_empty() {
                out.push_str("\n보기:\n");
                for (i, opt) in q.options.iter().enumerate() {
                    out.push_str(&format!("{}. {}\n", i + 1, opt));
                }
                out.push_str("\n답변(보기 중 하나만, 번호 또는 텍스트):");
            }
        }
        "multi" => {
            if !q.options.is_empty() {
                out.push_str("\n보기:\n");
                for (i, opt) in q.options.iter().enumerate() {
                    out.push_str(&format!("{}. {}\n", i + 1, opt));
                }
                out.push_str("\n답변(보기 중 1개 이상, 쉼표로 구분):");
            }
        }
        "scale" => {
            out.push_str(&format!(
                "\n척도: {}\n\n답변(숫자 하나):",
                q.scale.as_deref().unwrap_or("1=매우 그렇지 않다, 5=매우 그렇다")
            ));
        }
        _ => {
            out.push_str("\n답변(자유서술):");
        }
    }
    out
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn personas_run_survey(
    app: AppHandle,
    registry: State<'_, Arc<ChatRegistry>>,
    catalog_state: State<'_, Arc<CatalogState>>,
    llama_state: State<'_, LlamaServerState>,
    args: RunSurveyArgs,
    channel: Channel<PersonasSurveyEvent>,
) -> Result<(), PersonasSurveyError> {
    let total_calls = args.personas.len() * args.survey.questions.len();
    let _ = channel.send(PersonasSurveyEvent::Started { total_calls });

    let started = std::time::Instant::now();
    let cancel = CancellationToken::new();
    let _registry = (*registry).clone(); // 향후 cancel cascade 등에 활용 가능.

    let mut completed = 0usize;
    for persona in &args.personas {
        if cancel.is_cancelled() {
            let _ = channel.send(PersonasSurveyEvent::Cancelled);
            return Ok(());
        }
        let system_prompt = build_system_prompt(persona, args.system_extra.as_deref());

        for question in &args.survey.questions {
            if cancel.is_cancelled() {
                let _ = channel.send(PersonasSurveyEvent::Cancelled);
                return Ok(());
            }

            let user_prompt = build_user_prompt(question);
            let messages = vec![
                ChatMessage {
                    role: "system".into(),
                    content: system_prompt.clone(),
                    images: None,
                },
                ChatMessage {
                    role: "user".into(),
                    content: user_prompt,
                    images: None,
                },
            ];

            let _ = channel.send(PersonasSurveyEvent::Progress {
                completed,
                total: total_calls,
                current_persona: persona.uuid.clone(),
                current_question: question.id.clone(),
            });

            let q_started = std::time::Instant::now();
            let answer = match args.runtime_kind {
                RuntimeKind::Ollama => {
                    let adapter = OllamaAdapter::new();
                    let buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
                    let buf_c = buf.clone();
                    let outcome = adapter
                        .chat_stream(
                            &args.model_id,
                            &messages,
                            move |event: ChatEvent| {
                                if let ChatEvent::Delta { text } = event {
                                    if let Ok(mut b) = buf_c.lock() {
                                        b.push_str(&text);
                                    }
                                }
                            },
                            &cancel,
                        )
                        .await;
                    let collected = buf.lock().map(|b| b.clone()).unwrap_or_default();
                    match outcome {
                        ChatOutcome::Completed => collected,
                        ChatOutcome::Cancelled => {
                            let _ = channel.send(PersonasSurveyEvent::Cancelled);
                            return Ok(());
                        }
                        ChatOutcome::Failed(m) => {
                            let _ = channel.send(PersonasSurveyEvent::Failed {
                                message: format!("LLM 호출 실패: {m}"),
                            });
                            return Ok(());
                        }
                    }
                }
                RuntimeKind::LlamaCpp => {
                    // LlamaCpp single-instance — start_chat 패턴 재사용.
                    let entry = catalog_state
                        .snapshot()
                        .entries()
                        .iter()
                        .find(|e| e.id == args.model_id)
                        .cloned()
                        .ok_or_else(|| PersonasSurveyError::ModelNotReady {
                            message: format!("카탈로그에 모델이 없어요: {}", args.model_id),
                        })?;
                    let cache_dir = app
                        .path()
                        .app_local_data_dir()
                        .map_err(|e| PersonasSurveyError::Internal {
                            message: format!("dir: {e}"),
                        })?
                        .join("models");
                    let spec = build_server_spec(&entry, &cache_dir);
                    ensure_model_files_present(&spec).map_err(|e| {
                        PersonasSurveyError::ModelNotReady {
                            message: e.into_korean_message(),
                        }
                    })?;

                    let mut state = llama_state.lock().await;
                    let needs_spawn = match state.as_ref() {
                        Some(m) => m.model_path() != spec.model_path,
                        None => true,
                    };
                    if needs_spawn {
                        *state = None;
                        let handle = runner_llama_cpp::LlamaServerHandle::start(
                            spec.clone(),
                            cancel.clone(),
                        )
                        .await
                        .map_err(|e| PersonasSurveyError::ModelNotReady {
                            message: format!("llama-server 시작 실패: {e}"),
                        })?;
                        *state = Some(ManagedLlamaServer::new(handle, spec));
                    }
                    let endpoint = state.as_ref().unwrap().endpoint_base_url().to_string();
                    drop(state);

                    let adapter = adapter_llama_cpp::LlamaCppAdapter::with_endpoint(&endpoint);
                    let buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
                    let buf_c = buf.clone();
                    let outcome = adapter
                        .chat_stream(
                            &args.model_id,
                            &messages,
                            move |event: ChatEvent| {
                                if let ChatEvent::Delta { text } = event {
                                    if let Ok(mut b) = buf_c.lock() {
                                        b.push_str(&text);
                                    }
                                }
                            },
                            &cancel,
                        )
                        .await;
                    let collected = buf.lock().map(|b| b.clone()).unwrap_or_default();
                    match outcome {
                        ChatOutcome::Completed => collected,
                        ChatOutcome::Cancelled => {
                            let _ = channel.send(PersonasSurveyEvent::Cancelled);
                            return Ok(());
                        }
                        ChatOutcome::Failed(m) => {
                            let _ = channel.send(PersonasSurveyEvent::Failed {
                                message: format!("LLM 호출 실패: {m}"),
                            });
                            return Ok(());
                        }
                    }
                }
                other => {
                    return Err(PersonasSurveyError::UnsupportedRuntime {
                        runtime: format!("{other:?}").to_lowercase(),
                    });
                }
            };

            let took_ms = q_started.elapsed().as_millis() as u64;
            let _ = channel.send(PersonasSurveyEvent::Answer {
                answer: SurveyAnswer {
                    persona_uuid: persona.uuid.clone(),
                    question_id: question.id.clone(),
                    answer: answer.trim().to_string(),
                    took_ms,
                },
            });
            completed += 1;
        }
    }

    let _ = channel.send(PersonasSurveyEvent::Completed {
        count: completed,
        total_ms: started.elapsed().as_millis() as u64,
    });
    Ok(())
}
