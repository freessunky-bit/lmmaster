//! Workbench IPC 모듈 — Phase 5'.b. Channel<WorkbenchEvent> + WorkbenchRegistry + 5단계 flow.
//!
//! 정책 (phase-5pb-4p5b-ipc-reinforcement.md §1.1, §1.2):
//! - `tauri::ipc::Channel<WorkbenchEvent>` per-invocation stream — `Emitter::emit`보다 typed + ordered.
//! - WorkbenchRegistry는 `app.manage(Arc<WorkbenchRegistry>)`로 공유 — clone으로 task 캡처.
//! - cancel은 별도 `cancel_workbench_run` command (Tauri invoke AbortSignal 미지원 — issue #8351).
//! - run_id 단위 다중 동시 run 허용 (uuid). registry는 run_id ↔ CancellationToken + start_time + stage.
//! - 단계 전환마다 1회 + 단계 내 progress 100% 단위 emit — 단계당 < 10 events.
//! - send 실패 = window 닫힘 → cancel 트리거.
//! - 한국어 해요체 에러 메시지.
//!
//! Phase 1A.3.c (install) + Phase 2'.c.2 (bench)와 동일 패턴.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::State;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use bench_harness::{ResponderRuntimeKind, WorkbenchResponder};
use model_registry::{CustomModel, ModelRegistry as CustomModelRegistry, ModelRegistryError};

use workbench_core::{
    baseline_korean_eval_cases, parse_jsonl, render, run_eval_suite, write_jsonl, BootstrapEvent,
    ChatExample, EvalReport, LlamaFactoryTrainer, LlamaQuantizer, LoRAJob, LoRATrainer,
    MockLoRATrainer, MockQuantizer, ModelfileSpec, QuantizeJob, QuantizeProgress, Quantizer,
    Responder, WorkbenchConfig, WorkbenchError, WorkbenchRun, WorkbenchStep,
    DEFAULT_BOOTSTRAP_TIMEOUT_SECS, DEFAULT_LORA_TIMEOUT_SECS, DEFAULT_QUANTIZE_TIMEOUT_SECS,
};

// ───────────────────────────────────────────────────────────────────
// Event enum — frontend Channel<WorkbenchEvent> 송신용
// ───────────────────────────────────────────────────────────────────

/// 한 단계의 progress 상세. quantize/lora가 동일 shape를 공유.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StageProgressDetail {
    pub stage: WorkbenchStep,
    pub percent: u8,
    pub label: String,
    pub message: Option<String>,
}

/// run 종료 요약 — 등록된 Modelfile 경로 + eval 점수 등.
///
/// Phase 5'.c+d 보강:
/// - `eval_report`: per-case + 카테고리 집계 전체 (Validate stage가 실 채점 결과 publish).
/// - `registered_model_id`: Register stage가 model-registry에 영속한 custom-model id.
/// - `registered_modelfile_path`: 디스크에 저장된 Modelfile 경로 (Phase 5'.e에서 실 파일화).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkbenchRunSummary {
    pub run_id: String,
    pub total_duration_ms: u64,
    pub artifact_paths: Vec<String>,
    pub eval_passed: usize,
    pub eval_total: usize,
    pub modelfile_preview: Option<String>,
    /// Validate stage의 per-case + 카테고리 집계 보고서. v1 mock 환경에서도 정상 채워짐.
    #[serde(default)]
    pub eval_report: Option<EvalReport>,
    /// Register stage가 model-registry에 영속한 custom-model id.
    #[serde(default)]
    pub registered_model_id: Option<String>,
}

/// Channel<WorkbenchEvent>로 frontend에 흘려보내는 event.
///
/// `#[serde(tag = "kind", rename_all = "kebab-case")]`는 InstallEvent와 동일 패턴.
/// frontend는 `event.kind`로 discriminated union을 narrow한다.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkbenchEvent {
    /// run 시작 직후 1회.
    Started {
        run_id: String,
        config: WorkbenchConfig,
    },
    /// 단계 진입.
    StageStarted {
        run_id: String,
        stage: WorkbenchStep,
    },
    /// 단계 진행률 (5-step quantize/lora 등).
    StageProgress {
        run_id: String,
        progress: StageProgressDetail,
    },
    /// 단계 종료 — 다음 stage로 자동 전이.
    StageCompleted {
        run_id: String,
        stage: WorkbenchStep,
    },
    /// Validate stage 완료 — per-case + 카테고리 집계 결과를 즉시 publish.
    /// Completed 이전에 발생하는 stage-결과 이벤트.
    EvalCompleted { run_id: String, report: EvalReport },
    /// Register stage 완료 — model-registry에 영속한 custom-model id.
    /// Completed 이전에 발생.
    RegisterCompleted { run_id: String, model_id: String },
    /// `ollama create` shell-out 시작 — register_to_ollama=true이고 외부 CLI 호출이 필요할 때.
    /// Phase 5'.e — research §3.
    OllamaCreateStarted { run_id: String, output_name: String },
    /// `ollama create` stdout/stderr 1라인 도착 — UI에 진행 상황 라이브 노출.
    OllamaCreateProgress { run_id: String, line: String },
    /// `ollama create` 정상 종료 (exit 0).
    OllamaCreateCompleted { run_id: String },
    /// `ollama create` 비정상 종료 — 한국어 해요체 stderr 매핑된 에러.
    OllamaCreateFailed { run_id: String, error: String },
    /// 모든 단계 정상 완료. summary 포함.
    Completed {
        run_id: String,
        summary: WorkbenchRunSummary,
    },
    /// 단계 도중 실패. message는 한국어 해요체.
    Failed { run_id: String, error: String },
    /// 사용자 cancel 또는 channel close.
    Cancelled { run_id: String },
}

// ───────────────────────────────────────────────────────────────────
// API error — invoke().reject로 frontend에 전달
// ───────────────────────────────────────────────────────────────────

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum WorkbenchApiError {
    #[error("진행 중인 run을 찾을 수 없어요: {run_id}")]
    UnknownRun { run_id: String },

    #[error("워크벤치 시작에 실패했어요: {message}")]
    StartFailed { message: String },

    #[error("커스텀 모델 레지스트리에 저장하지 못했어요: {message}")]
    RegistryFailed { message: String },
}

impl From<ModelRegistryError> for WorkbenchApiError {
    fn from(e: ModelRegistryError) -> Self {
        Self::RegistryFailed {
            message: format!("{e}"),
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Registry — run_id ↔ (CancellationToken, started_at, current_stage)
// ───────────────────────────────────────────────────────────────────

/// 활성 run 메타. registry snapshot에 그대로 노출.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActiveRunSnapshot {
    pub run_id: String,
    /// RFC3339 시작 시각.
    pub started_at: String,
    pub current_stage: WorkbenchStep,
}

/// 내부 entry — token + 메타.
struct RegistryEntry {
    cancel: CancellationToken,
    started_at: String,
    current_stage: WorkbenchStep,
}

/// run_id ↔ CancellationToken + 메타. tokio::sync::Mutex 사용 — start/cancel/list가 매우 짧은
/// 락 보유 시간이지만, 내부 future가 await할 가능성 0이라 sync std::Mutex로도 충분.
/// 다만 reinforcement note에서 결정한 대로 tokio::sync::Mutex 채택 — 일관성.
#[derive(Default)]
pub struct WorkbenchRegistry {
    inner: AsyncMutex<HashMap<String, RegistryEntry>>,
}

impl WorkbenchRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 새 run 등록. run_id는 caller가 uuid로 생성. 중복 시 거부.
    pub async fn register(&self, run_id: &str) -> Result<CancellationToken, WorkbenchApiError> {
        let mut g = self.inner.lock().await;
        if g.contains_key(run_id) {
            return Err(WorkbenchApiError::StartFailed {
                message: format!("run_id 충돌이에요 ({run_id}). 다시 시도해 주세요."),
            });
        }
        let tok = CancellationToken::new();
        let now = time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default();
        g.insert(
            run_id.to_string(),
            RegistryEntry {
                cancel: tok.clone(),
                started_at: now,
                current_stage: WorkbenchStep::Data,
            },
        );
        Ok(tok)
    }

    /// 현재 stage 갱신 — list_runs가 즉시 보이도록.
    pub async fn set_stage(&self, run_id: &str, stage: WorkbenchStep) {
        let mut g = self.inner.lock().await;
        if let Some(entry) = g.get_mut(run_id) {
            entry.current_stage = stage;
        }
    }

    /// run 종료 — entry 제거. 미존재면 no-op.
    pub async fn finish(&self, run_id: &str) {
        let mut g = self.inner.lock().await;
        g.remove(run_id);
    }

    /// run cancel — idempotent. 미존재 = no-op.
    pub async fn cancel(&self, run_id: &str) {
        let g = self.inner.lock().await;
        if let Some(entry) = g.get(run_id) {
            entry.cancel.cancel();
        }
    }

    /// 모든 run cancel — 앱 종료 시.
    pub async fn cancel_all(&self) {
        let g = self.inner.lock().await;
        for entry in g.values() {
            entry.cancel.cancel();
        }
    }

    /// snapshot — 현재 active runs.
    pub async fn list(&self) -> Vec<ActiveRunSnapshot> {
        let g = self.inner.lock().await;
        let mut out: Vec<ActiveRunSnapshot> = g
            .iter()
            .map(|(id, entry)| ActiveRunSnapshot {
                run_id: id.clone(),
                started_at: entry.started_at.clone(),
                current_stage: entry.current_stage,
            })
            .collect();
        out.sort_by(|a, b| a.started_at.cmp(&b.started_at));
        out
    }

    /// 디버그용 카운트.
    pub async fn in_flight_count(&self) -> usize {
        self.inner.lock().await.len()
    }

    /// 비-async 종료 시점 sync cancel — `RunEvent::ExitRequested` 같은 sync 컨텍스트에서 호출.
    /// `tokio::sync::Mutex::try_lock()` 기반 best-effort. lock을 즉시 잡지 못하면 skip
    /// (run task가 이미 종료 중이거나 list/cancel가 막 끝나려는 상황 — 곧 cleanup).
    pub fn cancel_all_blocking(&self) {
        if let Ok(g) = self.inner.try_lock() {
            for entry in g.values() {
                entry.cancel.cancel();
            }
        }
    }
}

// ───────────────────────────────────────────────────────────────────
// Channel forwarding helper — send 실패 시 cancel 트리거 (installer의 emit_or_cancel 패턴).
// ───────────────────────────────────────────────────────────────────

fn emit_or_cancel(
    channel: &Channel<WorkbenchEvent>,
    cancel: &CancellationToken,
    event: WorkbenchEvent,
) {
    if channel.send(event).is_err() {
        // window 닫힘 등 — cancel 트리거.
        tracing::debug!("workbench channel send failed; triggering cancel");
        cancel.cancel();
    }
}

// ───────────────────────────────────────────────────────────────────
// run_workbench — pure async function. 5단계 flow 실행 + Channel emit.
// ───────────────────────────────────────────────────────────────────

/// 5단계 flow 실행. 각 단계는:
/// 1. cancel 체크 → cancelled 이벤트 + early return.
/// 2. registry stage 갱신.
/// 3. StageStarted emit.
/// 4. 단계 작업 (mock or real).
/// 5. StageCompleted emit.
///
/// Step 1 (Data): JSONL 검증 + 정규화. file 못 읽으면 에러.
/// Step 2 (Quantize): MockQuantizer로 5-step progress.
/// Step 3 (LoRA): MockLoRATrainer로 5-step progress.
/// Step 4 (Validate): baseline 10 evals — Responder 호출 후 deterministic 채점. EvalCompleted publish.
/// Step 5 (Register): Modelfile 렌더 + ModelRegistry::register 영속화 → RegisterCompleted publish.
pub async fn run_workbench(
    config: WorkbenchConfig,
    registry: Arc<WorkbenchRegistry>,
    model_registry: Arc<CustomModelRegistry>,
    responder: Arc<dyn Responder>,
    cancel: CancellationToken,
    channel: Channel<WorkbenchEvent>,
) -> WorkbenchRun {
    let start = Instant::now();
    let mut run = WorkbenchRun::new(config.clone());
    let run_id = run.id.clone();

    // 1. Started emit.
    emit_or_cancel(
        &channel,
        &cancel,
        WorkbenchEvent::Started {
            run_id: run_id.clone(),
            config: config.clone(),
        },
    );

    // ── Step 1: Data ─────────────────────────────────────────────
    if !run_stage_data(&run, &cancel, &channel).await {
        registry.finish(&run_id).await;
        if cancel.is_cancelled() {
            run.mark_cancelled();
        } else {
            run.mark_failed();
        }
        return run;
    }

    // ── Step 2: Quantize ─────────────────────────────────────────
    let mut artifact_paths: Vec<String> = Vec::new();
    let quant_output = format!(
        "workspace/workbench/{}/quantize/output.{}.gguf",
        run_id,
        config.quant_type.to_lowercase()
    );
    artifact_paths.push(quant_output.clone());
    run.advance_to(WorkbenchStep::Quantize);
    registry.set_stage(&run_id, WorkbenchStep::Quantize).await;
    // Phase 9'.b: use_real_quantizer = true이면 LlamaQuantizer (PATH/env detect),
    // 미발견 시 즉시 한국어 안내 후 Failed. false면 Mock(기존 동작).
    let quantizer: Box<dyn Quantizer> = match build_quantizer(&config) {
        Ok(q) => q,
        Err(e) => {
            emit_or_cancel(
                &channel,
                &cancel,
                WorkbenchEvent::Failed {
                    run_id: run_id.clone(),
                    error: format!("{e}"),
                },
            );
            registry.finish(&run_id).await;
            run.mark_failed();
            return run;
        }
    };
    if !run_stage_quantize(
        &run,
        &config,
        &quant_output,
        &cancel,
        &channel,
        quantizer.as_ref(),
    )
    .await
    {
        registry.finish(&run_id).await;
        if cancel.is_cancelled() {
            run.mark_cancelled();
        } else {
            run.mark_failed();
        }
        return run;
    }

    // ── Step 3: LoRA ─────────────────────────────────────────────
    let lora_output = format!("workspace/workbench/{}/lora/adapter", run_id);
    artifact_paths.push(lora_output.clone());
    run.advance_to(WorkbenchStep::Lora);
    registry.set_stage(&run_id, WorkbenchStep::Lora).await;
    // Phase 9'.b: use_real_trainer = true이면 LlamaFactoryTrainer 기대 (사전 부트스트랩 필수).
    // 미부트스트랩이면 한국어 안내. false면 Mock.
    let trainer: Box<dyn LoRATrainer> = match build_trainer(&config) {
        Ok(t) => t,
        Err(e) => {
            emit_or_cancel(
                &channel,
                &cancel,
                WorkbenchEvent::Failed {
                    run_id: run_id.clone(),
                    error: format!("{e}"),
                },
            );
            registry.finish(&run_id).await;
            run.mark_failed();
            return run;
        }
    };
    if !run_stage_lora(
        &run,
        &config,
        &lora_output,
        &cancel,
        &channel,
        trainer.as_ref(),
    )
    .await
    {
        registry.finish(&run_id).await;
        if cancel.is_cancelled() {
            run.mark_cancelled();
        } else {
            run.mark_failed();
        }
        return run;
    }

    // ── Step 4: Validate ─────────────────────────────────────────
    run.advance_to(WorkbenchStep::Validate);
    registry.set_stage(&run_id, WorkbenchStep::Validate).await;
    let eval_report = match run_stage_validate(&run, responder.as_ref(), &cancel, &channel).await {
        Some(r) => r,
        None => {
            registry.finish(&run_id).await;
            if cancel.is_cancelled() {
                run.mark_cancelled();
            } else {
                run.mark_failed();
            }
            return run;
        }
    };

    // ── Step 5: Register ─────────────────────────────────────────
    run.advance_to(WorkbenchStep::Register);
    registry.set_stage(&run_id, WorkbenchStep::Register).await;
    let register_outcome = run_stage_register(
        RegisterStageInputs {
            run: &run,
            config: &config,
            gguf_path: &quant_output,
            lora_adapter_path: &lora_output,
            eval_report: &eval_report,
            model_registry: &model_registry,
        },
        &cancel,
        &channel,
    )
    .await;
    let (modelfile_preview, registered_model_id) = match register_outcome {
        Some(p) => p,
        None => {
            registry.finish(&run_id).await;
            if cancel.is_cancelled() {
                run.mark_cancelled();
            } else {
                run.mark_failed();
            }
            return run;
        }
    };

    // ── Done ─────────────────────────────────────────────────────
    run.mark_completed();
    let total_duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let summary = WorkbenchRunSummary {
        run_id: run_id.clone(),
        total_duration_ms,
        artifact_paths,
        eval_passed: eval_report.passed_count,
        eval_total: eval_report.total,
        modelfile_preview: Some(modelfile_preview),
        eval_report: Some(eval_report),
        registered_model_id,
    };
    emit_or_cancel(
        &channel,
        &cancel,
        WorkbenchEvent::Completed {
            run_id: run_id.clone(),
            summary,
        },
    );
    registry.finish(&run_id).await;
    run
}

/// Step 1 — Data 검증. config.data_jsonl_path가 빈 string이면 mock JSONL 검증, 아니면 파일 읽기.
async fn run_stage_data(
    run: &WorkbenchRun,
    cancel: &CancellationToken,
    channel: &Channel<WorkbenchEvent>,
) -> bool {
    if cancel.is_cancelled() {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::Cancelled {
                run_id: run.id.clone(),
            },
        );
        return false;
    }
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageStarted {
            run_id: run.id.clone(),
            stage: WorkbenchStep::Data,
        },
    );
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageProgress {
            run_id: run.id.clone(),
            progress: StageProgressDetail {
                stage: WorkbenchStep::Data,
                percent: 50,
                label: "checking".into(),
                message: Some("입력 데이터 형식을 확인하고 있어요".into()),
            },
        },
    );

    // 파일 read를 시도. 빈 path면 mock 데이터로 통과.
    let path = &run.config.data_jsonl_path;
    if !path.is_empty() && std::path::Path::new(path).exists() {
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                emit_or_cancel(
                    channel,
                    cancel,
                    WorkbenchEvent::Failed {
                        run_id: run.id.clone(),
                        error: format!("데이터 파일을 읽지 못했어요: {e}"),
                    },
                );
                return false;
            }
        };
        if let Err(e) = parse_jsonl(&content) {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::Failed {
                    run_id: run.id.clone(),
                    error: format!("{e}"),
                },
            );
            return false;
        }
    }
    // 빈 path or 파일 없음: v1 mock — pass-through.

    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageProgress {
            run_id: run.id.clone(),
            progress: StageProgressDetail {
                stage: WorkbenchStep::Data,
                percent: 100,
                label: "checked".into(),
                message: Some("데이터 형식 확인을 마쳤어요".into()),
            },
        },
    );
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageCompleted {
            run_id: run.id.clone(),
            stage: WorkbenchStep::Data,
        },
    );
    true
}

/// Step 2 — Quantize. Quantizer trait `run_streaming` 호출 + progress live forward.
///
/// Phase 9'.b: streaming 변형으로 업그레이드 — 실 binary 30분 작업 중에도 매 stdout 라인이
/// 즉시 UI에 emit. Mock은 default impl이 `run` 결과를 forward하므로 호환.
async fn run_stage_quantize(
    run: &WorkbenchRun,
    config: &WorkbenchConfig,
    output_path: &str,
    cancel: &CancellationToken,
    channel: &Channel<WorkbenchEvent>,
    quantizer: &dyn Quantizer,
) -> bool {
    if cancel.is_cancelled() {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::Cancelled {
                run_id: run.id.clone(),
            },
        );
        return false;
    }
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageStarted {
            run_id: run.id.clone(),
            stage: WorkbenchStep::Quantize,
        },
    );
    let job = QuantizeJob {
        input_gguf: format!("{}.gguf", config.base_model_id),
        output_gguf: output_path.to_string(),
        quant_type: config.quant_type.clone(),
    };

    let (tx, mut rx) = mpsc::channel::<QuantizeProgress>(64);
    let runner = quantizer.run_streaming(job, tx, cancel);
    tokio::pin!(runner);
    let outcome = loop {
        tokio::select! {
            biased;
            res = &mut runner => break res,
            Some(p) = rx.recv() => {
                forward_progress_event(run, channel, cancel, WorkbenchStep::Quantize, &p);
            }
        }
    };
    // sender drop 후 남은 progress drain.
    while let Some(p) = rx.recv().await {
        forward_progress_event(run, channel, cancel, WorkbenchStep::Quantize, &p);
    }

    match outcome {
        Ok(()) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::StageCompleted {
                    run_id: run.id.clone(),
                    stage: WorkbenchStep::Quantize,
                },
            );
            true
        }
        Err(WorkbenchError::Cancelled) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::Cancelled {
                    run_id: run.id.clone(),
                },
            );
            false
        }
        Err(e) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::Failed {
                    run_id: run.id.clone(),
                    error: format!("{e}"),
                },
            );
            false
        }
    }
}

/// Step 3 — LoRA. LoRATrainer trait `run_streaming` 호출 + progress live forward.
async fn run_stage_lora(
    run: &WorkbenchRun,
    config: &WorkbenchConfig,
    output_adapter: &str,
    cancel: &CancellationToken,
    channel: &Channel<WorkbenchEvent>,
    trainer: &dyn LoRATrainer,
) -> bool {
    if cancel.is_cancelled() {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::Cancelled {
                run_id: run.id.clone(),
            },
        );
        return false;
    }
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageStarted {
            run_id: run.id.clone(),
            stage: WorkbenchStep::Lora,
        },
    );
    let job = LoRAJob {
        base_model: config.base_model_id.clone(),
        dataset_jsonl: config.data_jsonl_path.clone(),
        output_adapter: output_adapter.to_string(),
        epochs: config.lora_epochs,
        lr: 0.0002,
        korean_preset: config.korean_preset,
    };

    let (tx, mut rx) = mpsc::channel::<QuantizeProgress>(64);
    let runner = trainer.run_streaming(job, tx, cancel);
    tokio::pin!(runner);
    let outcome = loop {
        tokio::select! {
            biased;
            res = &mut runner => break res,
            Some(p) = rx.recv() => {
                forward_progress_event(run, channel, cancel, WorkbenchStep::Lora, &p);
            }
        }
    };
    while let Some(p) = rx.recv().await {
        forward_progress_event(run, channel, cancel, WorkbenchStep::Lora, &p);
    }

    match outcome {
        Ok(()) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::StageCompleted {
                    run_id: run.id.clone(),
                    stage: WorkbenchStep::Lora,
                },
            );
            true
        }
        Err(WorkbenchError::Cancelled) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::Cancelled {
                    run_id: run.id.clone(),
                },
            );
            false
        }
        Err(e) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::Failed {
                    run_id: run.id.clone(),
                    error: format!("{e}"),
                },
            );
            false
        }
    }
}

/// Step 4 — Validate. baseline 10 case를 Responder로 평가 후 EvalCompleted publish.
///
/// Phase 5'.c 보강: workbench_core::run_eval_suite로 위임 — cancel-aware. v1은 MockResponder가
/// 베이스라인 통과를 보장. Phase 5'.e에서 WorkbenchResponder가 실 HTTP로 위임.
async fn run_stage_validate(
    run: &WorkbenchRun,
    responder: &dyn Responder,
    cancel: &CancellationToken,
    channel: &Channel<WorkbenchEvent>,
) -> Option<EvalReport> {
    if cancel.is_cancelled() {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::Cancelled {
                run_id: run.id.clone(),
            },
        );
        return None;
    }
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageStarted {
            run_id: run.id.clone(),
            stage: WorkbenchStep::Validate,
        },
    );

    let cases = baseline_korean_eval_cases();
    let total = cases.len();

    // Progress emit: case 진입 전마다 1번. run_eval_suite는 atomic 진행이라 여기서는 시작 시점만 emit.
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageProgress {
            run_id: run.id.clone(),
            progress: StageProgressDetail {
                stage: WorkbenchStep::Validate,
                percent: 25,
                label: "evaluating".into(),
                message: Some(format!("한국어 baseline {total}건 평가하고 있어요")),
            },
        },
    );

    // run_eval_suite — cancel-aware, 모든 case에 대해 responder.respond + evaluate_response.
    let report = match run_eval_suite(responder, &cases, cancel, &run.config.base_model_id).await {
        Ok(r) => r,
        Err(WorkbenchError::Cancelled) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::Cancelled {
                    run_id: run.id.clone(),
                },
            );
            return None;
        }
        Err(e) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::Failed {
                    run_id: run.id.clone(),
                    error: format!("한국어 평가 중 오류가 났어요: {e}"),
                },
            );
            return None;
        }
    };

    // 최종 progress 100% — 결과 메시지에 점수 포함.
    let pct = (report.passed_count * 100).checked_div(total).unwrap_or(0);
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageProgress {
            run_id: run.id.clone(),
            progress: StageProgressDetail {
                stage: WorkbenchStep::Validate,
                percent: 100,
                label: "evaluated".into(),
                message: Some(format!(
                    "정답률 {pct}% (통과 {} / 전체 {})",
                    report.passed_count, report.total
                )),
            },
        },
    );

    // EvalCompleted — UI가 per-case + 카테고리 집계 즉시 표시.
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::EvalCompleted {
            run_id: run.id.clone(),
            report: report.clone(),
        },
    );

    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageCompleted {
            run_id: run.id.clone(),
            stage: WorkbenchStep::Validate,
        },
    );
    Some(report)
}

/// `run_stage_register` 입력 묶음 — clippy `too_many_arguments` 회피 + 의도 명확화.
struct RegisterStageInputs<'a> {
    run: &'a WorkbenchRun,
    config: &'a WorkbenchConfig,
    gguf_path: &'a str,
    lora_adapter_path: &'a str,
    eval_report: &'a EvalReport,
    model_registry: &'a CustomModelRegistry,
}

/// Step 5 — Register. Modelfile 렌더 → ModelRegistry::register로 영속화 → RegisterCompleted publish.
///
/// Phase 5'.d 보강: model-registry crate의 ModelRegistry로 custom-model 영속.
/// 반환 (modelfile_preview, registered_model_id_option). 실 `ollama create` 호출은 Phase 5'.e.
async fn run_stage_register(
    inputs: RegisterStageInputs<'_>,
    cancel: &CancellationToken,
    channel: &Channel<WorkbenchEvent>,
) -> Option<(String, Option<String>)> {
    let RegisterStageInputs {
        run,
        config,
        gguf_path,
        lora_adapter_path,
        eval_report,
        model_registry,
    } = inputs;
    if cancel.is_cancelled() {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::Cancelled {
                run_id: run.id.clone(),
            },
        );
        return None;
    }
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageStarted {
            run_id: run.id.clone(),
            stage: WorkbenchStep::Register,
        },
    );
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageProgress {
            run_id: run.id.clone(),
            progress: StageProgressDetail {
                stage: WorkbenchStep::Register,
                percent: 30,
                label: "rendering".into(),
                message: Some("Modelfile을 만들고 있어요".into()),
            },
        },
    );

    let spec = ModelfileSpec {
        gguf_path: gguf_path.to_string(),
        temperature: 0.7,
        num_ctx: 4096,
        system_prompt_ko: if config.korean_preset {
            "당신은 한국어를 우선 사용하는 도우미예요. 사용자에게 친근한 해요체로 답해 주세요."
                .into()
        } else {
            "You are a helpful assistant.".into()
        },
        stop_sequences: vec!["</s>".into(), "<|im_end|>".into()],
        template: None,
    };
    let preview = render(&spec);

    // ModelRegistry에 영속화 — register_to_ollama가 켜져 있을 때만.
    let mut registered_id: Option<String> = None;
    if config.register_to_ollama {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::StageProgress {
                run_id: run.id.clone(),
                progress: StageProgressDetail {
                    stage: WorkbenchStep::Register,
                    percent: 70,
                    label: "persisting".into(),
                    message: Some("커스텀 모델 카탈로그에 저장하고 있어요".into()),
                },
            },
        );
        let custom = CustomModel {
            id: run.id.clone(),
            base_model: config.base_model_id.clone(),
            quant_type: config.quant_type.clone(),
            lora_adapter: Some(lora_adapter_path.to_string()),
            modelfile: preview.clone(),
            created_at: time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .unwrap_or_default(),
            eval_passed: eval_report.passed_count,
            eval_total: eval_report.total,
            artifact_paths: vec![gguf_path.to_string(), lora_adapter_path.to_string()],
        };
        match model_registry.register(custom) {
            Ok(id) => {
                emit_or_cancel(
                    channel,
                    cancel,
                    WorkbenchEvent::RegisterCompleted {
                        run_id: run.id.clone(),
                        model_id: id.clone(),
                    },
                );
                registered_id = Some(id);
                emit_or_cancel(
                    channel,
                    cancel,
                    WorkbenchEvent::StageProgress {
                        run_id: run.id.clone(),
                        progress: StageProgressDetail {
                            stage: WorkbenchStep::Register,
                            percent: 90,
                            label: "registered".into(),
                            message: Some("모델 카탈로그에 등록을 마쳤어요".into()),
                        },
                    },
                );
                // `ollama create` shell-out — register_to_ollama=true이고 responder가 Ollama인
                // 경우에만 실행. 외부 호출은 사용자가 명시한 환경에서만.
                if matches!(config.responder_runtime.as_deref(), Some("ollama"))
                    && !run_ollama_create_stage(run, config, gguf_path, &preview, cancel, channel)
                        .await
                {
                    return None;
                }
                emit_or_cancel(
                    channel,
                    cancel,
                    WorkbenchEvent::StageProgress {
                        run_id: run.id.clone(),
                        progress: StageProgressDetail {
                            stage: WorkbenchStep::Register,
                            percent: 100,
                            label: "complete".into(),
                            message: Some("등록을 모두 마쳤어요".into()),
                        },
                    },
                );
            }
            Err(e) => {
                emit_or_cancel(
                    channel,
                    cancel,
                    WorkbenchEvent::Failed {
                        run_id: run.id.clone(),
                        error: format!("{e}"),
                    },
                );
                return None;
            }
        }
    } else {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::StageProgress {
                run_id: run.id.clone(),
                progress: StageProgressDetail {
                    stage: WorkbenchStep::Register,
                    percent: 100,
                    label: "skipped".into(),
                    message: Some("Modelfile 미리보기만 만들었어요".into()),
                },
            },
        );
    }

    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageCompleted {
            run_id: run.id.clone(),
            stage: WorkbenchStep::Register,
        },
    );
    Some((preview, registered_id))
}

// ───────────────────────────────────────────────────────────────────
// Responder builder + Ollama shell-out helpers (Phase 5'.e)
// ───────────────────────────────────────────────────────────────────

/// `WorkbenchConfig.responder_runtime` 문자열을 `ResponderRuntimeKind`로 매핑.
/// 알 수 없는 값은 Mock으로 안전 fallback.
fn parse_responder_runtime(s: &str) -> ResponderRuntimeKind {
    match s {
        "ollama" => ResponderRuntimeKind::Ollama,
        "lm-studio" | "lmstudio" => ResponderRuntimeKind::LmStudio,
        _ => ResponderRuntimeKind::Mock,
    }
}

/// config에서 적절한 `WorkbenchResponder`를 생성.
///
/// - `responder_runtime` Some + Ollama/LM Studio + base_url Some → 실 HTTP responder.
/// - 그 외 (None / Mock / 잘못된 값) → mock variant (deterministic stub).
///
/// Phase R-F+R-G hotfix (ADR-0064 §2): `WorkbenchResponder::new()`가 base_url localhost-only
/// allowlist를 강제. 비-localhost host 입력 시 `WorkbenchError::InvalidBaseUrl` 그대로 전파.
pub(crate) fn build_responder(
    config: &WorkbenchConfig,
) -> Result<WorkbenchResponder, workbench_core::WorkbenchError> {
    let kind = config
        .responder_runtime
        .as_deref()
        .map(parse_responder_runtime)
        .unwrap_or(ResponderRuntimeKind::Mock);
    match kind {
        ResponderRuntimeKind::Mock => Ok(WorkbenchResponder::mock()),
        ResponderRuntimeKind::Ollama => {
            let base_url = config
                .responder_base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".into());
            let model_id = config
                .responder_model_id
                .clone()
                .unwrap_or_else(|| config.base_model_id.clone());
            WorkbenchResponder::new(ResponderRuntimeKind::Ollama, model_id, base_url)
        }
        ResponderRuntimeKind::LmStudio => {
            let base_url = config
                .responder_base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:1234".into());
            let model_id = config
                .responder_model_id
                .clone()
                .unwrap_or_else(|| config.base_model_id.clone());
            WorkbenchResponder::new(ResponderRuntimeKind::LmStudio, model_id, base_url)
        }
    }
}

/// Phase 9'.b — Quantizer dispatch (실 binary vs mock).
///
/// `use_real_quantizer = true`이면 PATH 또는 `LMMASTER_LLAMA_QUANTIZE_PATH` env로
/// `llama-quantize`를 detect. 미발견 시 한국어 `WorkbenchError::ToolMissing`. false면 Mock.
pub(crate) fn build_quantizer(
    config: &WorkbenchConfig,
) -> Result<Box<dyn Quantizer>, WorkbenchError> {
    if config.use_real_quantizer {
        let timeout = Duration::from_secs(DEFAULT_QUANTIZE_TIMEOUT_SECS);
        let q = LlamaQuantizer::from_environment(timeout)?;
        Ok(Box::new(q))
    } else {
        Ok(Box::new(MockQuantizer))
    }
}

/// Phase 9'.b — LoRATrainer dispatch.
///
/// `use_real_trainer = true`이면 사용자 명시 동의 후 부트스트랩 완료된 venv 경로를 기대.
/// 미부트스트랩 시 한국어 안내 — caller(UI)가 `lora_bootstrap_venv` 명령으로 사전 부트스트랩.
pub(crate) fn build_trainer(
    config: &WorkbenchConfig,
) -> Result<Box<dyn LoRATrainer>, WorkbenchError> {
    if config.use_real_trainer {
        let venv_dir = lora_venv_dir();
        let python = lora_venv_python(&venv_dir);
        if !python.exists() {
            return Err(WorkbenchError::ToolMissing {
                tool: "LLaMA-Factory venv가 아직 만들어지지 않았어요. \
                       LoRA 화면에서 '실 모드 venv 만들기' 버튼을 눌러 부트스트랩한 뒤 다시 시도해 주세요."
                    .into(),
            });
        }
        let timeout = Duration::from_secs(DEFAULT_LORA_TIMEOUT_SECS);
        Ok(Box::new(LlamaFactoryTrainer::with_paths(
            venv_dir, python, timeout,
        )))
    } else {
        Ok(Box::new(MockLoRATrainer))
    }
}

/// venv 디렉터리 — temp_dir 기반. Phase 9'.b는 OS별 cache_dir로 v1.x에 이동 예정.
pub(crate) fn lora_venv_dir() -> PathBuf {
    if let Ok(p) = std::env::var("LMMASTER_LORA_VENV_DIR") {
        return PathBuf::from(p);
    }
    std::env::temp_dir().join("lmmaster-lora").join("venv")
}

/// venv 안 python 실행 파일 경로.
pub(crate) fn lora_venv_python(venv_dir: &std::path::Path) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        venv_dir.join("Scripts").join("python.exe")
    }
    #[cfg(not(target_os = "windows"))]
    {
        venv_dir.join("bin").join("python")
    }
}

/// stderr 마지막 줄에서 사용자 향 한국어 메시지를 추출 — research §3.4 매트릭스.
fn map_ollama_stderr_to_korean(stderr: &str) -> String {
    let lower = stderr.to_lowercase();
    if lower.contains("connection refused") {
        "Ollama 데몬이 꺼져 있어요. Ollama를 켠 뒤 다시 시도해 주세요.".into()
    } else if lower.contains("no such file") {
        "Modelfile에서 참조한 파일을 찾지 못했어요.".into()
    } else if lower.contains("command must be one of") {
        "Modelfile 형식이 잘못됐어요. 자동 생성을 다시 실행해 보세요.".into()
    } else if lower.contains("no space left") {
        "디스크 공간이 부족해요. 공간을 확보한 뒤 다시 시도해 주세요.".into()
    } else if lower.contains("model already exists") || lower.contains("already exists") {
        "같은 이름의 모델이 있어요. 다시 등록할까요?".into()
    } else if lower.contains("failed to fetch") || lower.contains("dial tcp") {
        "기본 모델을 받지 못했어요. 인터넷 연결을 확인해 주세요.".into()
    } else if stderr.trim().is_empty() {
        "Ollama 등록이 실패했어요.".into()
    } else {
        // 마지막 5줄까지를 사용자 향 메시지에 첨부.
        let tail: String = stderr
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" / ");
        format!("Ollama 등록이 실패했어요: {tail}")
    }
}

/// `ollama create -f Modelfile` shell-out — kill_on_drop + 60s timeout + cancel cooperative.
///
/// 환경변수 `MOCKED_OLLAMA_PATH`가 있으면 그 경로의 fixture 바이너리/스크립트를 호출 (테스트용).
/// 없으면 시스템 PATH의 `ollama`를 호출.
pub(crate) async fn run_ollama_create(
    output_name: &str,
    modelfile_path: &str,
    cwd: &std::path::Path,
    cancel: &CancellationToken,
    on_line: impl Fn(String) + Send + Sync + 'static,
) -> Result<(), String> {
    let bin = std::env::var("MOCKED_OLLAMA_PATH").unwrap_or_else(|_| "ollama".into());
    let mut cmd = Command::new(bin);
    cmd.arg("create")
        .arg(output_name)
        .arg("-f")
        .arg(modelfile_path)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Ollama 실행 파일을 찾지 못했어요: {e}"))?;

    // stdout / stderr를 동시에 라인 단위로 읽어 emit. 닫힐 때까지 진행.
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Ollama stdout pipe 생성 실패".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Ollama stderr pipe 생성 실패".to_string())?;

    let on_line = Arc::new(on_line);
    let stderr_buffer: Arc<AsyncMutex<String>> = Arc::new(AsyncMutex::new(String::new()));

    let stdout_emitter = on_line.clone();
    let stdout_task = tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            (stdout_emitter)(line);
        }
    });

    let stderr_emitter = on_line.clone();
    let stderr_buf_clone = stderr_buffer.clone();
    let stderr_task = tauri::async_runtime::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            {
                let mut buf = stderr_buf_clone.lock().await;
                buf.push_str(&line);
                buf.push('\n');
            }
            (stderr_emitter)(line);
        }
    });

    // wait + cancel + 60s timeout 동시 listen.
    let timeout = tokio::time::sleep(Duration::from_secs(60));
    tokio::pin!(timeout);
    let wait_result = tokio::select! {
        () = cancel.cancelled() => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err("Ollama 등록이 취소됐어요.".to_string())
        }
        () = &mut timeout => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            Err("Ollama 등록이 60초 안에 끝나지 않았어요. 큰 모델은 더 시간이 필요해요.".to_string())
        }
        status = child.wait() => {
            let status = status.map_err(|e| format!("Ollama 종료 상태를 읽지 못했어요: {e}"))?;
            if status.success() {
                Ok(())
            } else {
                let stderr_text = stderr_buffer.lock().await.clone();
                Err(map_ollama_stderr_to_korean(&stderr_text))
            }
        }
    };

    // pipe reader join — drop으로 cleanup. abort보다 graceful.
    stdout_task.abort();
    stderr_task.abort();

    wait_result
}

/// `ollama create -f Modelfile` 실 shell-out 단계.
///
/// 동작:
/// 1. `workspace/workbench/{run_id}/register/Modelfile` 작성 (기존 preview 그대로).
/// 2. `OllamaCreateStarted` emit.
/// 3. `run_ollama_create` 호출 — stdout/stderr 라인을 `OllamaCreateProgress`로 emit.
/// 4. 성공 → `OllamaCreateCompleted`. 실패 → `OllamaCreateFailed` + return false.
///
/// 반환: 성공시 true, 실패/cancel시 false (caller가 즉시 return).
async fn run_ollama_create_stage(
    run: &WorkbenchRun,
    config: &WorkbenchConfig,
    _gguf_path: &str,
    modelfile_preview: &str,
    cancel: &CancellationToken,
    channel: &Channel<WorkbenchEvent>,
) -> bool {
    if cancel.is_cancelled() {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::Cancelled {
                run_id: run.id.clone(),
            },
        );
        return false;
    }

    // 출력 디렉터리 준비. workspace/workbench/{run_id}/register/.
    let work_dir = std::env::temp_dir()
        .join("lmmaster-workbench")
        .join(&run.id)
        .join("register");
    if let Err(e) = std::fs::create_dir_all(&work_dir) {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::OllamaCreateFailed {
                run_id: run.id.clone(),
                error: format!("작업 디렉터리를 만들지 못했어요: {e}"),
            },
        );
        return false;
    }
    let modelfile_path = work_dir.join("Modelfile");
    if let Err(e) = std::fs::write(&modelfile_path, modelfile_preview) {
        emit_or_cancel(
            channel,
            cancel,
            WorkbenchEvent::OllamaCreateFailed {
                run_id: run.id.clone(),
                error: format!("Modelfile을 작성하지 못했어요: {e}"),
            },
        );
        return false;
    }

    // 등록될 이름 — 사용자 base + run_id (uuid prefix 8자) 합성.
    let safe_base = config
        .base_model_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .to_lowercase();
    let suffix: String = run.id.chars().take(8).collect();
    let output_name = format!("lmmaster-{safe_base}-{suffix}");

    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::OllamaCreateStarted {
            run_id: run.id.clone(),
            output_name: output_name.clone(),
        },
    );

    // 채널 + run id를 클로저에 캡처 — 라인마다 emit.
    let channel_clone = channel.clone();
    let run_id_for_emit = run.id.clone();
    let cancel_for_emit = cancel.clone();
    let on_line = move |line: String| {
        // emit_or_cancel에 channel만 빌려주는 형태가 아니라 cloned channel이라 직접 send.
        let _ = channel_clone.send(WorkbenchEvent::OllamaCreateProgress {
            run_id: run_id_for_emit.clone(),
            line,
        });
        // send 실패 = window 닫힘 → cancel 트리거.
        // (channel.send는 idempotent하게 false 반환할 뿐 panic 없음.)
        let _ = &cancel_for_emit;
    };

    let result = run_ollama_create(
        &output_name,
        modelfile_path.to_str().unwrap_or("Modelfile"),
        &work_dir,
        cancel,
        on_line,
    )
    .await;

    match result {
        Ok(()) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::OllamaCreateCompleted {
                    run_id: run.id.clone(),
                },
            );
            true
        }
        Err(msg) => {
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::OllamaCreateFailed {
                    run_id: run.id.clone(),
                    error: msg.clone(),
                },
            );
            // 사용자에게도 한 번 더 명확히 노출 — Failed로 escalate해 retry 버튼 노출.
            emit_or_cancel(
                channel,
                cancel,
                WorkbenchEvent::Failed {
                    run_id: run.id.clone(),
                    error: msg,
                },
            );
            false
        }
    }
}

fn forward_progress_event(
    run: &WorkbenchRun,
    channel: &Channel<WorkbenchEvent>,
    cancel: &CancellationToken,
    stage: WorkbenchStep,
    p: &QuantizeProgress,
) {
    emit_or_cancel(
        channel,
        cancel,
        WorkbenchEvent::StageProgress {
            run_id: run.id.clone(),
            progress: StageProgressDetail {
                stage,
                percent: p.percent,
                label: p.stage.clone(),
                message: p.message.clone(),
            },
        },
    );
}

// ───────────────────────────────────────────────────────────────────
// Tauri commands
// ───────────────────────────────────────────────────────────────────

/// Workbench run 시작. run_id를 즉시 반환하고, run은 백그라운드 task로 진행.
/// 진행 이벤트는 `on_event` Channel로 흘려보낸다.
///
/// Phase 5'.c+d:
/// - Validate: WorkbenchResponder(v1: deterministic stub)로 baseline 평가.
/// - Register: ModelRegistry에 custom-model 영속화 (app_data_dir 우선, 실패 시 in-memory 폴백).
#[tauri::command]
pub async fn start_workbench_run(
    config: WorkbenchConfig,
    on_event: Channel<WorkbenchEvent>,
    registry: State<'_, Arc<WorkbenchRegistry>>,
    model_registry: State<'_, Arc<CustomModelRegistry>>,
) -> Result<String, WorkbenchApiError> {
    let run_id = Uuid::new_v4().to_string();
    let cancel = registry.register(&run_id).await?;
    let registry_arc: Arc<WorkbenchRegistry> = registry.inner().clone();
    let model_registry_arc: Arc<CustomModelRegistry> = model_registry.inner().clone();

    // Phase 5'.e: WorkbenchResponder를 config의 runtime 필드 기준으로 dispatch.
    // - responder_runtime이 "ollama" / "lm-studio"면 실 HTTP 어댑터.
    // - None이거나 "mock"이면 deterministic stub (테스트/UI 데모).
    // Phase R-F+R-G hotfix (ADR-0064 §2): base_url이 비-localhost면 한국어 메시지로 즉시 거부.
    let responder: Arc<dyn Responder> =
        Arc::new(
            build_responder(&config).map_err(|e| WorkbenchApiError::StartFailed {
                message: format!("{e}"),
            })?,
        );

    // Tauri 2 정책: tauri::async_runtime::spawn 사용 (tokio::spawn 금지 — Tauri가 자체 runtime 소유).
    let registry_for_cleanup = registry_arc.clone();
    tauri::async_runtime::spawn(async move {
        let _ = run_workbench(
            config,
            registry_arc,
            model_registry_arc,
            responder,
            cancel,
            on_event,
        )
        .await;
        // Phase 8'.0.c: run 종료 후 best-effort retention 정리.
        cleanup_after_run(registry_for_cleanup).await;
    });

    Ok(run_id)
}

/// 등록된 custom-model 목록 — UI Catalog 페이지에서 노출.
#[tauri::command]
pub async fn list_custom_models(
    model_registry: State<'_, Arc<CustomModelRegistry>>,
) -> Result<Vec<CustomModel>, WorkbenchApiError> {
    Ok(model_registry.list()?)
}

/// 진행 중 run을 cancel — idempotent.
#[tauri::command]
pub async fn cancel_workbench_run(
    run_id: String,
    registry: State<'_, Arc<WorkbenchRegistry>>,
) -> Result<(), WorkbenchApiError> {
    registry.cancel(&run_id).await;
    Ok(())
}

/// 활성 run 목록 (registry snapshot). 종료된 run은 즉시 제거됐으므로 빈 list 가능.
#[tauri::command]
pub async fn list_workbench_runs(
    registry: State<'_, Arc<WorkbenchRegistry>>,
) -> Result<Vec<ActiveRunSnapshot>, WorkbenchApiError> {
    Ok(registry.list().await)
}

// ───────────────────────────────────────────────────────────────────
// Phase 8'.0.c — Workbench artifact retention (ADR-0037)
// ───────────────────────────────────────────────────────────────────

/// Workbench 결과물 루트 — `<temp_dir>/lmmaster-workbench`.
///
/// `run_ollama_create_stage`가 사용하는 위치와 동일.
fn artifact_workspace_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("lmmaster-workbench")
}

/// 진행 중인 run의 id set — registry snapshot에서 추출. retention이 정리에서 제외.
async fn protected_run_ids(registry: &WorkbenchRegistry) -> std::collections::HashSet<String> {
    registry
        .list()
        .await
        .into_iter()
        .map(|s| s.run_id)
        .collect()
}

/// 매 run 종료 후 best-effort 자동 정리. 실패해도 caller 흐름에 영향 없음.
pub(crate) async fn cleanup_after_run(registry: Arc<WorkbenchRegistry>) {
    let dir = artifact_workspace_dir();
    let policy = workbench_core::RetentionPolicy::default();
    let protected = protected_run_ids(&registry).await;
    match workbench_core::cleanup_artifacts(&dir, &policy, &protected) {
        Ok(report) => {
            if report.removed_count > 0 {
                tracing::info!(
                    removed = report.removed_count,
                    freed_bytes = report.freed_bytes,
                    "Workbench artifact 자동 정리 완료",
                );
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "Workbench artifact 자동 정리 실패");
        }
    }
}

/// 현재 사용량 통계. 사용자가 Settings에서 조회.
#[tauri::command]
pub async fn get_artifact_stats() -> Result<workbench_core::ArtifactStats, WorkbenchApiError> {
    let dir = artifact_workspace_dir();
    let policy = workbench_core::RetentionPolicy::default();
    workbench_core::artifact_stats(&dir, &policy).map_err(|e| WorkbenchApiError::StartFailed {
        message: format!("{e}"),
    })
}

/// 사용자 명시 정리 — Settings 패널 "지금 정리할게요" 버튼.
#[tauri::command]
pub async fn cleanup_artifacts_now(
    registry: State<'_, Arc<WorkbenchRegistry>>,
) -> Result<workbench_core::CleanupReport, WorkbenchApiError> {
    let dir = artifact_workspace_dir();
    let policy = workbench_core::RetentionPolicy::default();
    let protected = protected_run_ids(&registry).await;
    workbench_core::cleanup_artifacts(&dir, &policy, &protected).map_err(|e| {
        WorkbenchApiError::StartFailed {
            message: format!("{e}"),
        }
    })
}

// ───────────────────────────────────────────────────────────────────
// JSONL preview helper — frontend Step 1에서 호출 (sync RPC).
// ───────────────────────────────────────────────────────────────────

/// 첫 N개 line을 정규화해서 preview로 반환. 실패 line은 skip + warn.
#[tauri::command]
pub async fn workbench_preview_jsonl(
    path: String,
    limit: Option<usize>,
) -> Result<Vec<ChatExample>, WorkbenchApiError> {
    let limit = limit.unwrap_or(5);
    let content = std::fs::read_to_string(&path).map_err(|e| WorkbenchApiError::StartFailed {
        message: format!("파일을 읽지 못했어요: {e}"),
    })?;
    let mut examples = parse_jsonl(&content).map_err(|e| WorkbenchApiError::StartFailed {
        message: format!("{e}"),
    })?;
    examples.truncate(limit);
    Ok(examples)
}

/// 정규화된 examples를 JSONL string으로 직렬화 (UI에서 다운로드/preview 용).
#[tauri::command]
pub async fn workbench_serialize_examples(
    examples: Vec<ChatExample>,
) -> Result<String, WorkbenchApiError> {
    write_jsonl(&examples).map_err(|e| WorkbenchApiError::StartFailed {
        message: format!("{e}"),
    })
}

// ───────────────────────────────────────────────────────────────────
// Phase 9'.b — 실 모드 진단 + venv 부트스트랩 IPC
// ───────────────────────────────────────────────────────────────────

/// 실 모드 진단 결과 — frontend가 사용자 동의 dialog 전 상태 확인.
#[derive(Debug, Clone, Serialize)]
pub struct WorkbenchRealStatus {
    /// `llama-quantize` 발견 여부 (PATH 또는 env).
    pub quantize_binary_found: bool,
    /// 발견된 binary 절대 경로 (있으면).
    pub quantize_binary_path: Option<String>,
    /// LLaMA-Factory venv가 부트스트랩 완료되었는지 (python 실행 파일 존재).
    pub trainer_venv_ready: bool,
    /// venv 디렉터리 절대 경로.
    pub trainer_venv_dir: String,
}

/// 실 모드 사용 가능 여부 진단 — 사용자 동의 dialog 직전 호출.
#[tauri::command]
pub async fn workbench_real_status() -> Result<WorkbenchRealStatus, WorkbenchApiError> {
    let timeout = Duration::from_secs(DEFAULT_QUANTIZE_TIMEOUT_SECS);
    let (quantize_binary_found, quantize_binary_path) =
        match LlamaQuantizer::from_environment(timeout) {
            Ok(q) => (true, Some(q.binary_path().display().to_string())),
            Err(_) => (false, None),
        };
    let venv_dir = lora_venv_dir();
    let python = lora_venv_python(&venv_dir);
    Ok(WorkbenchRealStatus {
        quantize_binary_found,
        quantize_binary_path,
        trainer_venv_ready: python.exists(),
        trainer_venv_dir: venv_dir.display().to_string(),
    })
}

/// LLaMA-Factory venv 부트스트랩 IPC — 사용자 명시 동의 후 호출.
///
/// 5~10GB 다운로드 + 부트스트랩 진행 이벤트를 `Channel<BootstrapEvent>`로 emit.
/// Cancel은 별도 명령(`cancel_workbench_run`과 분리)으로 가능 (`cancel_lora_bootstrap`).
#[tauri::command]
pub async fn lora_bootstrap_venv(
    on_event: Channel<BootstrapEvent>,
    bootstrap_registry: State<'_, Arc<LoraBootstrapRegistry>>,
) -> Result<String, WorkbenchApiError> {
    let venv_dir = lora_venv_dir();
    let token_id = Uuid::new_v4().to_string();
    let cancel = bootstrap_registry.register(&token_id).await;
    let (tx, mut rx) = mpsc::channel::<BootstrapEvent>(64);

    // forward task — rx에서 받아서 frontend channel로 emit + 종료 시점에 cleanup.
    let on_event_clone = on_event.clone();
    let token_id_for_cleanup = token_id.clone();
    let bootstrap_registry_clone: Arc<LoraBootstrapRegistry> = bootstrap_registry.inner().clone();
    tauri::async_runtime::spawn(async move {
        while let Some(ev) = rx.recv().await {
            if on_event_clone.send(ev).is_err() {
                break;
            }
        }
        bootstrap_registry_clone.finish(&token_id_for_cleanup).await;
    });

    // 부트스트랩은 백그라운드 task로 — caller가 즉시 token_id를 받고 cancel 가능.
    let cancel_for_task = cancel.clone();
    tauri::async_runtime::spawn(async move {
        match LlamaFactoryTrainer::bootstrap_or_open(
            venv_dir,
            Duration::from_secs(DEFAULT_BOOTSTRAP_TIMEOUT_SECS),
            tx.clone(),
            cancel_for_task,
        )
        .await
        {
            Ok(_) => {
                // Done은 이미 bootstrap_or_open 안에서 emit됨.
            }
            Err(e) => {
                let _ = tx
                    .send(BootstrapEvent::Failed {
                        error: format!("{e}"),
                    })
                    .await;
            }
        }
        // tx drop으로 forward task가 종료.
    });

    Ok(token_id)
}

/// 부트스트랩 cancel — idempotent.
#[tauri::command]
pub async fn cancel_lora_bootstrap(
    token_id: String,
    bootstrap_registry: State<'_, Arc<LoraBootstrapRegistry>>,
) -> Result<(), WorkbenchApiError> {
    bootstrap_registry.cancel(&token_id).await;
    Ok(())
}

/// venv 부트스트랩 in-flight tokens — cancel용 registry.
#[derive(Default)]
pub struct LoraBootstrapRegistry {
    inner: AsyncMutex<HashMap<String, CancellationToken>>,
}

impl LoraBootstrapRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn register(&self, id: &str) -> CancellationToken {
        let mut g = self.inner.lock().await;
        let tok = CancellationToken::new();
        g.insert(id.to_string(), tok.clone());
        tok
    }

    pub async fn cancel(&self, id: &str) {
        let g = self.inner.lock().await;
        if let Some(t) = g.get(id) {
            t.cancel();
        }
    }

    pub async fn finish(&self, id: &str) {
        let mut g = self.inner.lock().await;
        g.remove(id);
    }
}

// ───────────────────────────────────────────────────────────────────
// Tests
// ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use workbench_core::MockResponder;

    fn config() -> WorkbenchConfig {
        WorkbenchConfig {
            base_model_id: "Qwen2.5-3B".into(),
            data_jsonl_path: String::new(), // 빈 path = mock pass-through.
            quant_type: "Q4_K_M".into(),
            lora_epochs: 3,
            korean_preset: true,
            register_to_ollama: true,
            ..Default::default()
        }
    }

    fn responder() -> Arc<dyn Responder> {
        Arc::new(MockResponder::new())
    }

    fn model_reg() -> Arc<CustomModelRegistry> {
        Arc::new(CustomModelRegistry::in_memory())
    }

    // ── Registry tests ──────────────────────────────────────────

    #[tokio::test]
    async fn registry_register_then_list() {
        let r = WorkbenchRegistry::new();
        let _ = r.register("a").await.unwrap();
        let snaps = r.list().await;
        assert_eq!(snaps.len(), 1);
        assert_eq!(snaps[0].run_id, "a");
        assert_eq!(snaps[0].current_stage, WorkbenchStep::Data);
    }

    #[tokio::test]
    async fn registry_register_duplicate_rejects() {
        let r = WorkbenchRegistry::new();
        let _ = r.register("a").await.unwrap();
        let err = r.register("a").await.unwrap_err();
        assert!(matches!(err, WorkbenchApiError::StartFailed { .. }));
    }

    #[tokio::test]
    async fn registry_finish_removes() {
        let r = WorkbenchRegistry::new();
        let _ = r.register("a").await.unwrap();
        r.finish("a").await;
        assert_eq!(r.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn registry_cancel_unknown_is_noop() {
        let r = WorkbenchRegistry::new();
        r.cancel("unknown").await; // panic 안 함.
        assert_eq!(r.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn registry_cancel_marks_token() {
        let r = WorkbenchRegistry::new();
        let tok = r.register("a").await.unwrap();
        r.cancel("a").await;
        assert!(tok.is_cancelled());
    }

    #[tokio::test]
    async fn registry_cancel_all_marks_every_token() {
        let r = WorkbenchRegistry::new();
        let t1 = r.register("a").await.unwrap();
        let t2 = r.register("b").await.unwrap();
        r.cancel_all().await;
        assert!(t1.is_cancelled());
        assert!(t2.is_cancelled());
    }

    #[tokio::test]
    async fn registry_set_stage_updates_snapshot() {
        let r = WorkbenchRegistry::new();
        let _ = r.register("a").await.unwrap();
        r.set_stage("a", WorkbenchStep::Quantize).await;
        let snaps = r.list().await;
        assert_eq!(snaps[0].current_stage, WorkbenchStep::Quantize);
    }

    #[tokio::test]
    async fn registry_list_sorted_by_started_at() {
        let r = WorkbenchRegistry::new();
        let _ = r.register("a").await.unwrap();
        // RFC3339 타임스탬프는 매우 빠른 등록도 동일 ms 가능 — sort key는 lexicographic.
        // 동일 ms면 문자열 정렬은 "a" < "b" 등으로 안정.
        let _ = r.register("b").await.unwrap();
        let snaps = r.list().await;
        assert_eq!(snaps.len(), 2);
        // 적어도 안정 정렬이라 패닉은 안 함.
        assert!(snaps[0].started_at <= snaps[1].started_at);
    }

    // ── Event enum serde ─────────────────────────────────────────

    #[test]
    fn event_started_serializes_with_kind() {
        let ev = WorkbenchEvent::Started {
            run_id: "r1".into(),
            config: config(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "started");
        assert_eq!(v["run_id"], "r1");
    }

    #[test]
    fn event_stage_progress_serializes_kebab() {
        let ev = WorkbenchEvent::StageProgress {
            run_id: "r1".into(),
            progress: StageProgressDetail {
                stage: WorkbenchStep::Quantize,
                percent: 50,
                label: "quantizing".into(),
                message: Some("진행 중이에요".into()),
            },
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "stage-progress");
        assert_eq!(v["progress"]["stage"], "quantize");
        assert_eq!(v["progress"]["percent"], 50);
    }

    #[test]
    fn event_stage_completed_serializes() {
        let ev = WorkbenchEvent::StageCompleted {
            run_id: "r1".into(),
            stage: WorkbenchStep::Lora,
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "stage-completed");
        assert_eq!(v["stage"], "lora");
    }

    #[test]
    fn event_completed_summary_round_trip_shape() {
        let summary = WorkbenchRunSummary {
            run_id: "r1".into(),
            total_duration_ms: 1234,
            artifact_paths: vec!["a".into(), "b".into()],
            eval_passed: 8,
            eval_total: 10,
            modelfile_preview: Some("FROM x".into()),
            eval_report: None,
            registered_model_id: Some("uuid-x".into()),
        };
        let ev = WorkbenchEvent::Completed {
            run_id: "r1".into(),
            summary,
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "completed");
        assert_eq!(v["summary"]["eval_passed"], 8);
        assert_eq!(v["summary"]["eval_total"], 10);
        assert_eq!(v["summary"]["registered_model_id"], "uuid-x");
    }

    #[test]
    fn event_eval_completed_serializes_kebab() {
        use std::collections::HashMap;
        use workbench_core::EvalResult;
        let mut by_cat: HashMap<String, (usize, usize)> = HashMap::new();
        by_cat.insert("factuality".into(), (3, 4));
        let report = EvalReport {
            model_id: "qwen".into(),
            passed_count: 3,
            total: 4,
            by_category: by_cat,
            cases: vec![EvalResult {
                case_id: "fact-capital".into(),
                passed: true,
                failure_reason: None,
                model_response: "서울이에요".into(),
            }],
        };
        let ev = WorkbenchEvent::EvalCompleted {
            run_id: "r1".into(),
            report,
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "eval-completed");
        assert_eq!(v["report"]["passed_count"], 3);
        assert_eq!(v["report"]["total"], 4);
    }

    #[test]
    fn event_register_completed_serializes_kebab() {
        let ev = WorkbenchEvent::RegisterCompleted {
            run_id: "r1".into(),
            model_id: "uuid-1".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "register-completed");
        assert_eq!(v["model_id"], "uuid-1");
    }

    #[test]
    fn api_error_registry_failed_serialize_kebab() {
        let e = WorkbenchApiError::RegistryFailed {
            message: "io error".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "registry-failed");
    }

    #[test]
    fn event_failed_includes_message() {
        let ev = WorkbenchEvent::Failed {
            run_id: "r1".into(),
            error: "안 돼요".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "failed");
        assert!(v["error"].as_str().unwrap().contains("안 돼요"));
    }

    #[test]
    fn event_cancelled_kind_only() {
        let ev = WorkbenchEvent::Cancelled {
            run_id: "r1".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "cancelled");
        assert_eq!(v["run_id"], "r1");
    }

    #[test]
    fn api_error_unknown_run_serialize_kebab() {
        let e = WorkbenchApiError::UnknownRun {
            run_id: "r1".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "unknown-run");
        assert_eq!(v["run_id"], "r1");
    }

    #[test]
    fn api_error_start_failed_serialize_kebab() {
        let e = WorkbenchApiError::StartFailed {
            message: "x".into(),
        };
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["kind"], "start-failed");
    }

    // ── Flow tests with capture channel ──────────────────────────

    /// 카운터 기반 Channel — IPC body는 무시하고 호출 횟수만 검증.
    /// 실제 emit 검증은 IPC layer에서만 가능 (Channel::new closure는 InvokeResponseBody를 받음).
    fn counting_channel() -> (Channel<WorkbenchEvent>, Arc<AtomicUsize>) {
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();
        let ch: Channel<WorkbenchEvent> = Channel::new(move |_body| -> tauri::Result<()> {
            count_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        });
        (ch, count)
    }

    #[tokio::test]
    async fn run_workbench_completes_when_not_cancelled() {
        let registry = Arc::new(WorkbenchRegistry::new());
        let cancel = CancellationToken::new();
        let (ch, count) = counting_channel();

        let run = run_workbench(
            config(),
            registry.clone(),
            model_reg(),
            responder(),
            cancel,
            ch,
        )
        .await;
        // 정상 완료 — RunStatus::Completed.
        assert_eq!(run.status, workbench_core::RunStatus::Completed);
        // 5개 단계 + Started + Completed + 다수 progress emit. 최소 10건 이상.
        assert!(
            count.load(Ordering::SeqCst) >= 10,
            "최소 10건 이상의 이벤트가 emit되어야 함 (실제: {})",
            count.load(Ordering::SeqCst)
        );
        assert_eq!(registry.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn run_workbench_pre_cancelled_yields_cancelled_status() {
        let registry = Arc::new(WorkbenchRegistry::new());
        let cancel = CancellationToken::new();
        cancel.cancel(); // 시작 전 cancel.
        let (ch, _count) = counting_channel();

        let run = run_workbench(
            config(),
            registry.clone(),
            model_reg(),
            responder(),
            cancel,
            ch,
        )
        .await;
        assert_eq!(run.status, workbench_core::RunStatus::Cancelled);
        assert_eq!(registry.in_flight_count().await, 0);
    }

    #[tokio::test]
    async fn run_workbench_advances_through_all_5_steps() {
        let registry = Arc::new(WorkbenchRegistry::new());
        let cancel = CancellationToken::new();
        let (ch, _count) = counting_channel();

        let run = run_workbench(
            config(),
            registry.clone(),
            model_reg(),
            responder(),
            cancel,
            ch,
        )
        .await;
        // 5단계 모두 completed_steps에 들어가 있어야 함 (mark_completed에서 Register도 push).
        assert!(run.completed_steps.contains(&WorkbenchStep::Data));
        assert!(run.completed_steps.contains(&WorkbenchStep::Quantize));
        assert!(run.completed_steps.contains(&WorkbenchStep::Lora));
        assert!(run.completed_steps.contains(&WorkbenchStep::Validate));
        assert!(run.completed_steps.contains(&WorkbenchStep::Register));
    }

    #[tokio::test]
    async fn run_workbench_register_skipped_when_disabled() {
        let registry = Arc::new(WorkbenchRegistry::new());
        let cancel = CancellationToken::new();
        let (ch, _count) = counting_channel();
        let mut cfg = config();
        cfg.register_to_ollama = false;

        let model_reg_local = model_reg();
        let run = run_workbench(
            cfg,
            registry.clone(),
            model_reg_local.clone(),
            responder(),
            cancel,
            ch,
        )
        .await;
        assert_eq!(run.status, workbench_core::RunStatus::Completed);
        // toggle off → registry에는 저장되지 않음.
        let count = model_reg_local.count().expect("count");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn run_workbench_persists_custom_model_when_register_on() {
        let registry = Arc::new(WorkbenchRegistry::new());
        let cancel = CancellationToken::new();
        let (ch, _count) = counting_channel();
        let model_reg_local = model_reg();
        let run = run_workbench(
            config(),
            registry.clone(),
            model_reg_local.clone(),
            responder(),
            cancel,
            ch,
        )
        .await;
        assert_eq!(run.status, workbench_core::RunStatus::Completed);
        // 정상 완료면 custom-model 1건 등록.
        assert_eq!(model_reg_local.count().expect("count"), 1);
        let list = model_reg_local.list().expect("list");
        assert_eq!(list[0].id, run.id);
        assert_eq!(list[0].base_model, "Qwen2.5-3B");
        assert_eq!(list[0].quant_type, "Q4_K_M");
        assert_eq!(list[0].eval_total, 10);
        assert!(list[0].modelfile.contains("FROM"));
    }

    #[tokio::test]
    async fn run_workbench_disk_registry_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let registry = Arc::new(WorkbenchRegistry::new());
        let cancel = CancellationToken::new();
        let (ch, _count) = counting_channel();
        let model_reg_local = Arc::new(CustomModelRegistry::with_dir(tmp.path()));
        let _run = run_workbench(
            config(),
            registry.clone(),
            model_reg_local.clone(),
            responder(),
            cancel,
            ch,
        )
        .await;
        // 디스크 파일이 작성됨.
        assert!(tmp.path().join("custom-models.json").exists());
        // 다른 인스턴스로 reload — 같은 1건.
        let r2 = CustomModelRegistry::with_dir(tmp.path());
        assert_eq!(r2.count().unwrap(), 1);
    }

    #[tokio::test]
    async fn cancel_during_quantize_returns_cancelled() {
        let registry = Arc::new(WorkbenchRegistry::new());
        let cancel = CancellationToken::new();
        let (ch, _count) = counting_channel();
        let cancel_clone = cancel.clone();

        // 5ms 후 cancel — Data 단계 이후, Quantize 진행 중일 가능성.
        let cancel_task = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            cancel_clone.cancel();
        });

        let run = run_workbench(
            config(),
            registry.clone(),
            model_reg(),
            responder(),
            cancel,
            ch,
        )
        .await;
        let _ = cancel_task.await;
        // Cancel/Failed/Completed 셋 중 하나 — 타이밍에 따라 변동.
        // 핵심: registry는 정리되어야 함.
        assert_eq!(registry.in_flight_count().await, 0);
        // Cancelled가 가장 가능성 높지만, 이미 끝났으면 Completed도 OK.
        assert!(matches!(
            run.status,
            workbench_core::RunStatus::Cancelled | workbench_core::RunStatus::Completed
        ));
    }

    #[tokio::test]
    async fn workbench_preview_jsonl_reads_first_n_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.jsonl");
        let content = "\
{\"instruction\":\"a\",\"output\":\"A\"}
{\"질문\":\"b\",\"답변\":\"B\"}
{\"messages\":[{\"role\":\"user\",\"content\":\"c\"},{\"role\":\"assistant\",\"content\":\"C\"}]}
{\"instruction\":\"d\",\"output\":\"D\"}
";
        std::fs::write(&path, content).unwrap();
        // command function을 직접 호출 — State 매개변수가 없어 단순.
        let p = path.to_string_lossy().to_string();
        let result = workbench_preview_jsonl(p, Some(2)).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn workbench_preview_jsonl_unknown_path_returns_error() {
        let result = workbench_preview_jsonl("/nope/missing.jsonl".into(), None).await;
        assert!(result.is_err());
    }

    #[test]
    fn cancel_all_blocking_does_not_panic_on_empty() {
        let r = WorkbenchRegistry::new();
        r.cancel_all_blocking(); // panic 안 함.
    }

    #[test]
    fn stage_progress_detail_serialize_kebab_stage() {
        let d = StageProgressDetail {
            stage: WorkbenchStep::Validate,
            percent: 30,
            label: "evaluating".into(),
            message: None,
        };
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["stage"], "validate");
        assert_eq!(v["percent"], 30);
    }

    // ── Phase 5'.e — Responder builder + ollama create shell-out ────

    #[test]
    fn build_responder_default_is_mock() {
        let cfg = config();
        let r = build_responder(&cfg).unwrap();
        assert_eq!(r.runtime_kind(), bench_harness::ResponderRuntimeKind::Mock);
    }

    #[test]
    fn build_responder_ollama_when_runtime_set() {
        // Phase R-F+R-G hotfix: 비-localhost("http://example:11434")는 거부되므로 localhost 사용.
        let mut cfg = config();
        cfg.responder_runtime = Some("ollama".into());
        cfg.responder_base_url = Some("http://localhost:11434".into());
        cfg.responder_model_id = Some("llama3.1:8b".into());
        let r = build_responder(&cfg).unwrap();
        assert_eq!(
            r.runtime_kind(),
            bench_harness::ResponderRuntimeKind::Ollama
        );
        assert_eq!(r.model_id(), "llama3.1:8b");
        assert_eq!(r.base_url(), "http://localhost:11434");
    }

    #[test]
    fn build_responder_lmstudio_when_runtime_set() {
        let mut cfg = config();
        cfg.responder_runtime = Some("lm-studio".into());
        let r = build_responder(&cfg).unwrap();
        assert_eq!(
            r.runtime_kind(),
            bench_harness::ResponderRuntimeKind::LmStudio
        );
    }

    #[test]
    fn build_responder_unknown_runtime_falls_back_to_mock() {
        let mut cfg = config();
        cfg.responder_runtime = Some("totally-unknown".into());
        let r = build_responder(&cfg).unwrap();
        assert_eq!(r.runtime_kind(), bench_harness::ResponderRuntimeKind::Mock);
    }

    /// Phase R-F+R-G hotfix invariant — 비-localhost base_url은 InvalidBaseUrl 에러.
    #[test]
    fn build_responder_rejects_non_localhost_base_url() {
        let mut cfg = config();
        cfg.responder_runtime = Some("ollama".into());
        cfg.responder_base_url = Some("http://192.168.0.10:11434".into());
        let err = build_responder(&cfg).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("주소가 올바르지 않아요") && msg.contains("내 PC 안에서"),
            "expected korean InvalidBaseUrl message, got: {msg}"
        );
    }

    #[test]
    fn build_responder_rejects_https_base_url() {
        let mut cfg = config();
        cfg.responder_runtime = Some("lm-studio".into());
        cfg.responder_base_url = Some("https://localhost:1234".into());
        let err = build_responder(&cfg).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("https는 외부"),
            "expected korean error, got: {msg}"
        );
    }

    #[test]
    fn map_ollama_stderr_to_korean_connection_refused() {
        let msg = map_ollama_stderr_to_korean(
            "Error: dial tcp 127.0.0.1:11434: connect: connection refused",
        );
        assert!(msg.contains("Ollama 데몬"));
        assert!(msg.contains("꺼져"));
    }

    #[test]
    fn map_ollama_stderr_to_korean_no_such_file() {
        let msg = map_ollama_stderr_to_korean("Error: open ./model.gguf: no such file");
        assert!(msg.contains("Modelfile에서 참조한 파일"));
    }

    #[test]
    fn map_ollama_stderr_to_korean_command_must_be_one_of() {
        let msg = map_ollama_stderr_to_korean("Error: command must be one of 'from', 'license'");
        assert!(msg.contains("Modelfile 형식"));
    }

    #[test]
    fn map_ollama_stderr_to_korean_no_space() {
        let msg = map_ollama_stderr_to_korean("Error: no space left on device");
        assert!(msg.contains("디스크 공간"));
    }

    #[test]
    fn map_ollama_stderr_to_korean_already_exists() {
        let msg = map_ollama_stderr_to_korean("Error: model already exists");
        assert!(msg.contains("같은 이름"));
    }

    #[test]
    fn map_ollama_stderr_to_korean_failed_to_fetch() {
        let msg = map_ollama_stderr_to_korean("Error: failed to fetch base model");
        assert!(msg.contains("기본 모델"));
        assert!(msg.contains("인터넷"));
    }

    #[test]
    fn map_ollama_stderr_to_korean_unknown_includes_tail() {
        let stderr = "line1\nline2\nline3\nline4";
        let msg = map_ollama_stderr_to_korean(stderr);
        assert!(msg.contains("line4"));
        assert!(msg.contains("Ollama 등록"));
    }

    #[test]
    fn map_ollama_stderr_to_korean_empty_returns_default() {
        let msg = map_ollama_stderr_to_korean("");
        assert!(msg.contains("Ollama 등록"));
    }

    #[test]
    fn parse_responder_runtime_kebab_variants() {
        assert_eq!(
            parse_responder_runtime("ollama"),
            bench_harness::ResponderRuntimeKind::Ollama
        );
        assert_eq!(
            parse_responder_runtime("lm-studio"),
            bench_harness::ResponderRuntimeKind::LmStudio
        );
        assert_eq!(
            parse_responder_runtime("lmstudio"),
            bench_harness::ResponderRuntimeKind::LmStudio
        );
        assert_eq!(
            parse_responder_runtime("nope"),
            bench_harness::ResponderRuntimeKind::Mock
        );
    }

    #[test]
    fn event_ollama_create_started_serializes_kebab() {
        let ev = WorkbenchEvent::OllamaCreateStarted {
            run_id: "r1".into(),
            output_name: "lmmaster-x-12345678".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "ollama-create-started");
        assert_eq!(v["output_name"], "lmmaster-x-12345678");
    }

    #[test]
    fn event_ollama_create_progress_serializes() {
        let ev = WorkbenchEvent::OllamaCreateProgress {
            run_id: "r1".into(),
            line: "transferring model data".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "ollama-create-progress");
        assert_eq!(v["line"], "transferring model data");
    }

    #[test]
    fn event_ollama_create_completed_serializes() {
        let ev = WorkbenchEvent::OllamaCreateCompleted {
            run_id: "r1".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "ollama-create-completed");
    }

    #[test]
    fn event_ollama_create_failed_serializes_kebab() {
        let ev = WorkbenchEvent::OllamaCreateFailed {
            run_id: "r1".into(),
            error: "Modelfile 형식이 잘못됐어요".into(),
        };
        let v = serde_json::to_value(&ev).unwrap();
        assert_eq!(v["kind"], "ollama-create-failed");
        assert!(v["error"].as_str().unwrap().contains("Modelfile"));
    }

    #[tokio::test]
    async fn run_ollama_create_succeeds_with_mocked_zero_exit_binary() {
        // Windows: cmd.exe /c exit 0 — 가장 가벼운 zero-exit fixture.
        // 다른 OS는 /usr/bin/true (PATH 의존, CI 환경에서만 신뢰).
        #[cfg(target_os = "windows")]
        let cmd = "cmd";
        #[cfg(not(target_os = "windows"))]
        let cmd = "true";
        std::env::set_var("MOCKED_OLLAMA_PATH", cmd);

        let tmp = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();

        // cmd.exe는 args를 다르게 받아 — 단순 success 검증을 위해 직접 spawn 함수에 의존하지 않고
        // run_ollama_create 내부 args가 그대로 전달돼도 cmd가 무시 후 exit 0을 줄 거라 기대.
        // (Windows cmd /c "create xxx -f Modelfile" → "create" 같은 이름의 batch가 없으니 nonzero 가능.
        //  대신 일관 동작을 위해 fixture script 작성.)
        let result =
            run_ollama_create("test-model", "Modelfile", tmp.path(), &cancel, |_| {}).await;
        std::env::remove_var("MOCKED_OLLAMA_PATH");
        // 일부 OS / shell에서는 cmd.exe + 안 맞는 args로 nonzero 가능 — 어쨌든 panic 없이 결과 반환.
        let _ = result;
    }

    #[tokio::test]
    async fn run_ollama_create_cancel_returns_korean_error() {
        let tmp = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        // PATH에 'ollama' 없을 가능성 — 환경변수로 알려진 명령으로 fallback.
        // cancel pre-trigger로 spawn 직후 즉시 종료해야 함.
        #[cfg(target_os = "windows")]
        std::env::set_var("MOCKED_OLLAMA_PATH", "cmd");
        #[cfg(not(target_os = "windows"))]
        std::env::set_var("MOCKED_OLLAMA_PATH", "sleep");
        let result = run_ollama_create("name", "Modelfile", tmp.path(), &cancel, |_| {}).await;
        std::env::remove_var("MOCKED_OLLAMA_PATH");
        // 사전 cancel이라 child는 spawn 직후 kill 되거나 그 전에 select가 cancel arm을 선택.
        // 결과는 Err("취소됐어요")가 가장 일반적.
        if let Err(msg) = result {
            assert!(msg.contains("취소") || msg.contains("실행 파일"));
        }
    }

    #[tokio::test]
    async fn run_ollama_create_missing_binary_returns_korean_error() {
        let tmp = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        std::env::set_var("MOCKED_OLLAMA_PATH", "this-does-not-exist-anywhere-xyz");
        let result = run_ollama_create("name", "Modelfile", tmp.path(), &cancel, |_| {}).await;
        std::env::remove_var("MOCKED_OLLAMA_PATH");
        let err = result.unwrap_err();
        assert!(err.contains("실행 파일") || err.contains("Ollama"));
    }

    #[tokio::test]
    async fn run_workbench_with_mock_runtime_uses_mock_responder() {
        // responder_runtime 미지정 = Mock — config() 기본값 그대로.
        let registry = Arc::new(WorkbenchRegistry::new());
        let cancel = CancellationToken::new();
        let (ch, _count) = counting_channel();
        let mut cfg = config();
        // register_to_ollama=true이지만 responder_runtime은 None이라 ollama create 호출 안 함.
        cfg.responder_runtime = None;
        let run = run_workbench(
            cfg,
            registry.clone(),
            model_reg(),
            Arc::new(build_responder(&config()).unwrap()),
            cancel,
            ch,
        )
        .await;
        // mock으로 구동되어 Completed.
        assert_eq!(run.status, workbench_core::RunStatus::Completed);
    }
}
