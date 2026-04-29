//! End-to-end 5단계 mock 흐름 검증.
//!
//! 목적: workbench-core scaffold가 Data → Quantize → LoRA → Validate → Register 순서로
//! 결합 가능하고, 각 단계 trait이 mock impl과 100% 호환됨을 입증.

use tokio_util::sync::CancellationToken;

use workbench_core::{
    aggregate_with_cases, baseline_korean_eval_cases, evaluate_response, parse_jsonl, render,
    EvalResult, LoRAJob, LoRATrainer, MockLoRATrainer, MockQuantizer, ModelfileSpec, QuantizeJob,
    Quantizer, RunStatus, WorkbenchConfig, WorkbenchError, WorkbenchRun, WorkbenchStep,
};

fn config() -> WorkbenchConfig {
    WorkbenchConfig {
        base_model_id: "Qwen2.5-3B".into(),
        data_jsonl_path: "./data/mixed.jsonl".into(),
        quant_type: "Q4_K_M".into(),
        lora_epochs: 4,
        korean_preset: true,
        register_to_ollama: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn full_5_step_mock_run_completes() {
    // ── 0. Run 시작 ────────────────────────────────────────────────
    let mut run = WorkbenchRun::new(config());
    assert_eq!(run.current_step, WorkbenchStep::Data);
    assert_eq!(run.status, RunStatus::Pending);

    // ── 1. Data 단계: 4 포맷 mixed JSONL → 모두 정규화 통과 ─────
    let mixed_jsonl = "\
{\"instruction\":\"한국의 수도는?\",\"output\":\"서울입니다.\"}
{\"conversations\":[{\"from\":\"human\",\"value\":\"안녕\"},{\"from\":\"gpt\",\"value\":\"안녕하세요\"}]}
{\"messages\":[{\"role\":\"user\",\"content\":\"hi\"},{\"role\":\"assistant\",\"content\":\"hello\"}]}
{\"질문\":\"세종대왕이 만든 글자는?\",\"답변\":\"한글\"}
";
    let examples = parse_jsonl(mixed_jsonl).unwrap();
    assert_eq!(examples.len(), 4, "4 mixed-format lines should parse");
    for ex in &examples {
        assert!(ex.messages.iter().any(|m| m.role == "user"));
        assert!(ex.messages.iter().any(|m| m.role == "assistant"));
    }

    // ── 2. Quantize 단계 ───────────────────────────────────────────
    run.advance_to(WorkbenchStep::Quantize);
    assert_eq!(run.status, RunStatus::Running);
    assert_eq!(run.completed_steps, vec![WorkbenchStep::Data]);

    let quantizer = MockQuantizer;
    let cancel = CancellationToken::new();
    let q_progress = quantizer
        .run(
            QuantizeJob {
                input_gguf: "./models/base.gguf".into(),
                output_gguf: "./models/base-q4_k_m.gguf".into(),
                quant_type: run.config.quant_type.clone(),
            },
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(q_progress.len(), 5, "MockQuantizer should emit 5 stages");
    assert_eq!(q_progress.last().unwrap().percent, 100);

    // ── 3. LoRA 단계 ───────────────────────────────────────────────
    run.advance_to(WorkbenchStep::Lora);
    let trainer = MockLoRATrainer;
    let l_progress = trainer
        .run(
            LoRAJob {
                base_model: run.config.base_model_id.clone(),
                dataset_jsonl: run.config.data_jsonl_path.clone(),
                output_adapter: "./out/adapter".into(),
                epochs: run.config.lora_epochs,
                lr: 0.0002,
                korean_preset: run.config.korean_preset,
            },
            &cancel,
        )
        .await
        .unwrap();
    assert_eq!(l_progress.len(), 5);
    // korean_preset = true 이므로 두 번째 stage message에 "한국어" 키워드.
    let train_msg = l_progress[1].message.as_ref().unwrap();
    assert!(
        train_msg.contains("한국어"),
        "korean_preset=true should mention 한국어, got: {train_msg}"
    );
    assert!(train_msg.contains("alpaca-ko"));

    // ── 4. Validate 단계 ───────────────────────────────────────────
    run.advance_to(WorkbenchStep::Validate);
    let cases = baseline_korean_eval_cases();
    assert_eq!(cases.len(), 10);

    // 10 case에 대한 가짜 응답 — 일부 pass, 일부 fail로 by_category 검증.
    let fake_responses: Vec<&str> = vec![
        "수도는 서울이에요.",           // fact-capital → pass
        "한글이에요.",                  // fact-hangul → pass
        "잘 모르겠어요.",               // fact-last-king → fail (expected '순종' missing)
        "한국 전쟁이에요.",             // fact-1950 → pass
        "안녕하세요. 저는 도우미예요.", // inst-haeyo → pass
        "삼, 오, 칠.",                  // inst-numbers-only → pass
        "안녕하세요. 세계.",            // inst-translate-ko-only → pass
        "이걸 추천해요.",               // tone-no-question-pohoming → pass
        "지금 시각을 알려드릴게요.",    // tone-no-formal → pass
        "로딩 중이에요.",               // tone-no-bank-english → pass
    ];
    let results: Vec<EvalResult> = cases
        .iter()
        .zip(fake_responses.iter())
        .map(|(case, response)| evaluate_response(case, response))
        .collect();

    let report = aggregate_with_cases("test-model", results, &cases);
    assert_eq!(report.total, 10);
    assert!(
        report.passed_count >= 8,
        "expected at least 8 pass, got {}",
        report.passed_count
    );
    // 카테고리 3개 모두 존재.
    assert!(report.by_category.contains_key("factuality"));
    assert!(report.by_category.contains_key("instruction-following"));
    assert!(report.by_category.contains_key("tone-korean"));
    // factuality는 4 total 중 3 pass (last-king fail).
    let fact = report.by_category.get("factuality").unwrap();
    assert_eq!(fact.1, 4);
    assert_eq!(fact.0, 3);

    // ── 5. Register 단계 ───────────────────────────────────────────
    run.advance_to(WorkbenchStep::Register);
    let modelfile = render(&ModelfileSpec {
        gguf_path: "./models/base-q4_k_m-lora.gguf".into(),
        temperature: 0.7,
        num_ctx: 4096,
        system_prompt_ko: "한국어 해요체로 친절하게 답해 주세요.".into(),
        stop_sequences: vec!["</s>".into(), "<|im_end|>".into()],
        template: None,
    });
    assert!(modelfile.contains("FROM "));
    assert!(modelfile.contains("PARAMETER temperature 0.7"));
    assert!(modelfile.contains("PARAMETER num_ctx 4096"));
    assert!(modelfile.contains("PARAMETER stop \"</s>\""));
    assert!(modelfile.contains("PARAMETER stop \"<|im_end|>\""));
    assert!(modelfile.contains("SYSTEM "));
    assert!(modelfile.contains("한국어 해요체"));

    // ── 마무리: mark_completed ─────────────────────────────────────
    run.mark_completed();
    assert_eq!(run.status, RunStatus::Completed);
    // 5 steps 모두 completed.
    assert_eq!(run.completed_steps.len(), 5);
    assert!(run.completed_steps.contains(&WorkbenchStep::Data));
    assert!(run.completed_steps.contains(&WorkbenchStep::Quantize));
    assert!(run.completed_steps.contains(&WorkbenchStep::Lora));
    assert!(run.completed_steps.contains(&WorkbenchStep::Validate));
    assert!(run.completed_steps.contains(&WorkbenchStep::Register));
}

#[tokio::test]
async fn cancel_during_quantize_returns_cancelled_error() {
    let mut run = WorkbenchRun::new(config());
    run.advance_to(WorkbenchStep::Quantize);

    let quantizer = MockQuantizer;
    let cancel = CancellationToken::new();
    cancel.cancel(); // 시작 전 취소.

    let err = quantizer
        .run(
            QuantizeJob {
                input_gguf: "./x.gguf".into(),
                output_gguf: "./y.gguf".into(),
                quant_type: "Q4_K_M".into(),
            },
            &cancel,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, WorkbenchError::Cancelled));

    run.mark_cancelled();
    assert_eq!(run.status, RunStatus::Cancelled);
}

#[tokio::test]
async fn cancel_during_lora_returns_cancelled_error() {
    let trainer = MockLoRATrainer;
    let cancel = CancellationToken::new();
    cancel.cancel();
    let err = trainer
        .run(
            LoRAJob {
                base_model: "x".into(),
                dataset_jsonl: "./d.jsonl".into(),
                output_adapter: "./o".into(),
                epochs: 1,
                lr: 0.0001,
                korean_preset: false,
            },
            &cancel,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, WorkbenchError::Cancelled));
}

#[tokio::test]
async fn jsonl_skips_malformed_lines_in_data_stage() {
    let content = "\
{\"instruction\":\"a\",\"output\":\"A\"}
not even json
{\"질문\":\"b\",\"답변\":\"B\"}
{}
{\"messages\":[{\"role\":\"user\",\"content\":\"c\"},{\"role\":\"assistant\",\"content\":\"C\"}]}
";
    let examples = parse_jsonl(content).unwrap();
    // 3 valid lines (line 2 is not JSON, line 4 is unknown format).
    assert_eq!(examples.len(), 3);
}

#[test]
fn workbench_run_serde_round_trip_after_5_advances() {
    let mut run = WorkbenchRun::new(config());
    run.advance_to(WorkbenchStep::Quantize);
    run.advance_to(WorkbenchStep::Lora);
    run.advance_to(WorkbenchStep::Validate);
    run.advance_to(WorkbenchStep::Register);
    run.mark_completed();

    let s = serde_json::to_string(&run).unwrap();
    let parsed: WorkbenchRun = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed, run);
}
