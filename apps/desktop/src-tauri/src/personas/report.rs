//! 설문 결과 → 외부 LLM(ChatGPT/Gemini/Claude)에 붙여넣을 전문 리서치 리포트 프롬프트 생성.
//!
//! 정책:
//! - 사용자가 결과 화면에서 스타일 선택 (mckinsey | nielsen | academic).
//! - frontend가 응답 데이터 + persona 분포 + 설문 메타를 보내면 backend가 마크다운 프롬프트 생성.
//! - 프롬프트는 클립보드 복사 → 외부 LLM 붙여넣기 → 한국 엘리트 리서치 기관 수준 출력.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PersonasReportError {
    #[error("내부 오류: {message}")]
    Internal { message: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReportRequest {
    pub survey_title: String,
    pub persona_count: usize,
    /// 페르소나 분포 한 줄 요약 (예: "여 60% / 남 40% · 20대 30% / 30대 50% / 40대 20%").
    pub persona_distribution: String,
    /// 질문별 응답 집계 — frontend가 client-side로 통계 만들어 전달.
    pub question_summaries: Vec<QuestionSummary>,
    /// 출력 스타일.
    pub style: ReportStyle,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QuestionSummary {
    pub id: String,
    pub text: String,
    /// "single" | "multi" | "scale" | "open"
    #[serde(rename = "type")]
    pub q_type: String,
    /// 객관식: 보기별 응답 수. 빈 vec이면 미지원.
    #[serde(default)]
    pub option_counts: Vec<OptionCount>,
    /// 척도: 평균.
    #[serde(default)]
    pub scale_mean: Option<f64>,
    /// 주관식: 응답 샘플 (3~5건).
    #[serde(default)]
    pub open_samples: Vec<String>,
    /// 주관식 키워드 빈도 (top 10).
    #[serde(default)]
    pub open_keyword_freq: Vec<KeywordFreq>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OptionCount {
    pub option: String,
    pub count: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeywordFreq {
    pub keyword: String,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReportStyle {
    Mckinsey,
    Nielsen,
    Academic,
}

fn render_question(q: &QuestionSummary) -> String {
    let mut s = String::new();
    s.push_str(&format!("### {} — {}\n\n", q.id, q.text));
    match q.q_type.as_str() {
        "single" | "multi" => {
            if !q.option_counts.is_empty() {
                let total: usize = q.option_counts.iter().map(|o| o.count).sum();
                s.push_str("| 보기 | 응답 수 | 비율 |\n|---|---:|---:|\n");
                for o in &q.option_counts {
                    let pct = if total > 0 {
                        (o.count as f64 * 100.0 / total as f64).round()
                    } else {
                        0.0
                    };
                    s.push_str(&format!("| {} | {} | {:.0}% |\n", o.option, o.count, pct));
                }
                s.push('\n');
            }
        }
        "scale" => {
            if let Some(m) = q.scale_mean {
                s.push_str(&format!("- 평균 점수: **{:.2}**\n\n", m));
            }
        }
        _ => {
            if !q.open_samples.is_empty() {
                s.push_str("주요 응답 샘플:\n");
                for sample in &q.open_samples {
                    s.push_str(&format!("- \"{}\"\n", sample.replace('\n', " ").chars().take(200).collect::<String>()));
                }
                s.push('\n');
            }
            if !q.open_keyword_freq.is_empty() {
                s.push_str("키워드 빈도 (상위):\n");
                for k in &q.open_keyword_freq {
                    s.push_str(&format!("- {} ({})\n", k.keyword, k.count));
                }
                s.push('\n');
            }
        }
    }
    s
}

fn style_directive(style: ReportStyle) -> &'static str {
    match style {
        ReportStyle::Mckinsey => {
            "당신은 맥킨지(McKinsey) 시니어 컨설턴트예요. 위 데이터를 다음 구조의 한국어 리포트로 가공해 주세요:\n\
            \n\
            1. **Executive Summary** — 3 bullet, '그래서 무엇을 할 것인가'에 직답.\n\
            2. **Key Insights** — 3~5개 가설 (MECE 적용, 각 가설마다 데이터 근거 인용).\n\
            3. **Segmentation Findings** — 페르소나 세그먼트별(연령·성별) 차이.\n\
            4. **Strategic Implications** — 의사결정용 권고 3개 (impact × effort 매트릭스 포함).\n\
            5. **Suggested Charts** — 위 인사이트를 표현할 차트 제안 (Plotly/Chart.js JSON 코드 또는 Mermaid 다이어그램으로 즉시 렌더 가능하게).\n\
            6. **Risks & Caveats** — 본 시뮬레이션 데이터의 한계.\n\
            \n\
            톤: 한국어 전문 컨설팅 보고서. 명확·간결·근거 기반. 추측 금지."
        }
        ReportStyle::Nielsen => {
            "당신은 닐슨(Nielsen) UX 리서치 디렉터예요. 위 데이터를 다음 구조의 한국어 UX 리포트로 가공해 주세요:\n\
            \n\
            1. **Research Question** — 본 설문이 답하려는 가설.\n\
            2. **Methodology** — 페르소나 N명 시뮬, 한계 명시.\n\
            3. **Top Findings** — 5~7개, 각각 데이터 인용 + 사용자 인용(있다면).\n\
            4. **Persona Segments** — 응답 패턴이 다른 세그먼트 식별 + 각 세그먼트의 멘탈 모델.\n\
            5. **UX Recommendations** — Severity rating(1~5)과 함께 3~5개 액션.\n\
            6. **Visual Suggestions** — 인포그래픽/차트 SVG 코드 또는 D3.js 스니펫.\n\
            \n\
            톤: 사용자 중심·실행 가능·근거 명시. 마케팅 추천이 아닌 UX 의사결정용."
        }
        ReportStyle::Academic => {
            "당신은 한국 사회과학 분야 박사급 연구자예요. 위 데이터를 다음 구조의 학술 보고서 초안으로 가공해 주세요:\n\
            \n\
            1. **초록(Abstract)** — 한국어 200자, 영문 100단어.\n\
            2. **연구 배경 및 가설** — 선행 연구 필요 시 추가 조사 권고만 (가짜 인용 금지).\n\
            3. **연구 방법** — 합성 페르소나 시뮬, 표본 분포, 한계.\n\
            4. **분석 결과** — 질문별 정량 결과를 표·그래프(LaTeX/PGF 또는 Plotly JSON)로.\n\
            5. **논의(Discussion)** — 결과의 의미, 실무 시사점.\n\
            6. **한계 및 후속 연구 제안**.\n\
            7. **참고문헌** — 기존 인용 가능한 표준 출처만 (날조 금지).\n\
            \n\
            톤: 학술 한국어, 객관적·신중. 가짜 통계·인용 절대 금지."
        }
    }
}

#[tauri::command]
pub fn personas_generate_report_prompt(
    req: ReportRequest,
) -> Result<String, PersonasReportError> {
    Ok(build_full_prompt(&req))
}

fn build_full_prompt(req: &ReportRequest) -> String {
    let mut out = String::new();
    out.push_str("# 가상 페르소나 설문 시뮬레이션 결과\n\n");
    out.push_str(&format!("- **설문**: {}\n", req.survey_title));
    out.push_str(&format!(
        "- **응답자 수**: {} (가상 페르소나)\n",
        req.persona_count
    ));
    out.push_str(&format!("- **분포**: {}\n", req.persona_distribution));
    out.push_str("- **출처**: NVIDIA Nemotron-Personas-Korea (CC BY 4.0) + 사용자 PC의 로컬 LLM\n\n");
    out.push_str("---\n\n## 설문 결과\n\n");
    for q in &req.question_summaries {
        out.push_str(&render_question(q));
    }
    out.push_str("---\n\n## 가공 요청\n\n");
    out.push_str(style_directive(req.style));
    out.push_str("\n\n출력은 마크다운으로, 한국어로, 추가 설명 없이 본문만 작성해 주세요.");
    out
}

// ── v0.8.4 — Chunked Map-Reduce 리포트 프롬프트 ──────────────────────

/// 외부 LLM(ChatGPT/Claude/Gemini) 입력 한도 보수적 상한 (토큰).
/// 디폴트 80K — GPT-4o(128K) / Claude 3.5(200K) / Gemini 1.5(2M) 모두 안전.
const DEFAULT_CHUNK_TOKEN_LIMIT: u64 = 80_000;
/// 한 청크에 사용 가능한 비율 (지시문·헤더 여유 20%).
const CHUNK_USE_RATIO_NUM: u64 = 80;
const CHUNK_USE_RATIO_DEN: u64 = 100;

/// 토큰 추정 휴리스틱 — 한국어 1글자 ≈ 2.5 토큰 (보수적 상한).
///
/// 영문/숫자만 있는 경우 over-estimate(안전), 한국어가 섞이면 정확.
/// `tiktoken-rs` 같은 정밀 카운터를 도입하지 않은 이유는 결정 노트 §2.3 참조.
fn estimate_tokens(text: &str) -> u64 {
    // chars().count() × 25 / 10 = chars × 2.5 (정수 산술).
    (text.chars().count() as u64).saturating_mul(25) / 10
}

/// 청크 1개 — 사용자가 외부 LLM에 paste할 단일 prompt.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReportPromptChunk {
    /// 1-based 순번.
    pub seq: usize,
    /// 총 청크 수 — UI "1/N" 표시용.
    pub total: usize,
    /// 헤더 + 부분 데이터 + 부분 분석 지시 포함 prompt.
    pub prompt: String,
    /// 휴리스틱 토큰 추정.
    pub estimated_tokens: u64,
}

/// 리포트 프롬프트 계획 — 단일 또는 다중 청크.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ReportPromptPlan {
    /// 청크 N개. `total == 1`이면 single-shot — 기존 흐름과 동등.
    pub chunks: Vec<ReportPromptChunk>,
    /// 모든 청크 paste 후 사용자가 별도로 보낼 종합 합성 prompt.
    /// 청크 1개일 땐 None (불필요).
    pub final_synthesis: Option<String>,
    /// 모든 청크 합산 추정 토큰 (UI 카피용).
    pub estimated_tokens_total: u64,
}

fn build_chunk_header(seq: usize, total: usize, req: &ReportRequest) -> String {
    let mut h = String::new();
    if total > 1 {
        h.push_str(&format!(
            "## 부분 분석 요청 — 전체 {total}개 청크 중 {seq}번째\n\n"
        ));
        h.push_str(
            "이 메시지는 큰 설문 결과의 *일부*예요. 나머지 청크는 별도로 보낼게요.\n\
            지금은 *이 청크의 데이터*만 분석해 주세요. 종합은 마지막에 별도 요청드릴게요.\n\n",
        );
    } else {
        h.push_str("## 단일 분석 요청\n\n");
    }
    h.push_str(&format!("- **설문**: {}\n", req.survey_title));
    h.push_str(&format!(
        "- **응답자 수**: {} (가상 페르소나)\n",
        req.persona_count
    ));
    h.push_str(&format!("- **분포**: {}\n", req.persona_distribution));
    h.push_str("- **출처**: NVIDIA Nemotron-Personas-Korea (CC BY 4.0) + 사용자 PC의 로컬 LLM\n\n");
    h
}

fn build_chunk_footer(total: usize, style: ReportStyle) -> String {
    if total > 1 {
        // 부분 분석 — 합성은 마지막에 별도.
        "---\n\n## 부분 분석 지시\n\n\
        이 청크의 데이터에 대해 다음을 한국어로 정리해 주세요:\n\
        1. 핵심 발견 3~5개 (각 발견에 데이터 인용).\n\
        2. 주목할 만한 응답 패턴 / 이상치.\n\
        3. (있다면) 페르소나 세그먼트별 차이.\n\n\
        *전체 종합 보고서는 모든 청크 분석을 마친 후 별도로 요청드릴게요.* 지금은 이 청크만 정리해 주세요.\n"
            .to_string()
    } else {
        // 단일 — 풀 리포트 즉시.
        let mut f = String::from("---\n\n## 가공 요청\n\n");
        f.push_str(style_directive(style));
        f.push_str("\n\n출력은 마크다운으로, 한국어로, 추가 설명 없이 본문만 작성해 주세요.");
        f
    }
}

fn build_final_synthesis(req: &ReportRequest, total_chunks: usize) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "## 종합 합성 요청 — {total_chunks}개 부분 분석을 하나의 보고서로\n\n"
    ));
    s.push_str(
        "앞서 보낸 부분 분석 결과들을 *모두 종합*해, 다음 양식의 한국어 통합 보고서로 작성해 주세요.\n\n",
    );
    s.push_str(&format!("- **설문**: {}\n", req.survey_title));
    s.push_str(&format!(
        "- **총 응답자 수**: {} (가상 페르소나)\n",
        req.persona_count
    ));
    s.push_str(&format!("- **분포**: {}\n\n", req.persona_distribution));
    s.push_str("---\n\n## 가공 요청\n\n");
    s.push_str(style_directive(req.style));
    s.push_str(
        "\n\n중요: 위 부분 분석들의 *모든* 발견을 빠짐없이 통합하고, 청크 간 모순이 있으면 명시해 주세요.\n",
    );
    s.push_str("출력은 마크다운으로, 한국어로, 추가 설명 없이 본문만 작성해 주세요.");
    s
}

/// 질문 단위로 청크 분할 — 한 질문은 한 청크 안에 통째로.
///
/// 알고리즘:
/// 1. 헤더+푸터의 고정 토큰을 한도에서 제외.
/// 2. 질문을 순서대로 누적, 한도 80% 초과 시 새 청크 시작.
/// 3. 단일 질문이 한도를 초과해도 자체 청크 1개로 보존 (분할하지 않음).
fn build_plan_internal(req: &ReportRequest, chunk_token_limit: u64) -> ReportPromptPlan {
    // 1. 질문별 렌더 캐시 + 토큰.
    let rendered: Vec<(String, u64)> = req
        .question_summaries
        .iter()
        .map(|q| {
            let r = render_question(q);
            let t = estimate_tokens(&r);
            (r, t)
        })
        .collect();

    // 2. 청크 한도 (헤더+푸터 여유 20% 제외).
    let usable_per_chunk = chunk_token_limit
        .saturating_mul(CHUNK_USE_RATIO_NUM)
        / CHUNK_USE_RATIO_DEN;

    // 3. 청크 grouping (질문 순서 보존).
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut current_tokens: u64 = 0;
    for (idx, (_r, t)) in rendered.iter().enumerate() {
        if !current.is_empty() && current_tokens.saturating_add(*t) > usable_per_chunk {
            groups.push(std::mem::take(&mut current));
            current_tokens = 0;
        }
        current.push(idx);
        current_tokens = current_tokens.saturating_add(*t);
    }
    if !current.is_empty() {
        groups.push(current);
    }
    if groups.is_empty() {
        // 질문 0개 — 빈 단일 청크라도 1개 (헤더/footer만).
        groups.push(Vec::new());
    }

    let total = groups.len();

    // 4. 청크 prompt 빌드.
    let chunks: Vec<ReportPromptChunk> = groups
        .iter()
        .enumerate()
        .map(|(i, idx_list)| {
            let seq = i + 1;
            let mut p = String::new();
            p.push_str(&build_chunk_header(seq, total, req));
            p.push_str("---\n\n## 설문 결과\n\n");
            for q_idx in idx_list {
                p.push_str(&rendered[*q_idx].0);
            }
            p.push_str(&build_chunk_footer(total, req.style));
            let estimated_tokens = estimate_tokens(&p);
            ReportPromptChunk {
                seq,
                total,
                prompt: p,
                estimated_tokens,
            }
        })
        .collect();

    // 5. 다중 청크면 합성 prompt 생성.
    let final_synthesis = if total > 1 {
        Some(build_final_synthesis(req, total))
    } else {
        None
    };

    let estimated_tokens_total: u64 = chunks
        .iter()
        .map(|c| c.estimated_tokens)
        .sum::<u64>()
        .saturating_add(
            final_synthesis
                .as_deref()
                .map(estimate_tokens)
                .unwrap_or(0),
        );

    ReportPromptPlan {
        chunks,
        final_synthesis,
        estimated_tokens_total,
    }
}

#[tauri::command]
pub fn personas_generate_report_prompt_plan(
    req: ReportRequest,
) -> Result<ReportPromptPlan, PersonasReportError> {
    Ok(build_plan_internal(&req, DEFAULT_CHUNK_TOKEN_LIMIT))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_summary(id: &str, text: &str) -> QuestionSummary {
        QuestionSummary {
            id: id.into(),
            text: text.into(),
            q_type: "open".into(),
            option_counts: Vec::new(),
            scale_mean: None,
            open_samples: vec!["응답 샘플입니다".into(); 5],
            open_keyword_freq: Vec::new(),
        }
    }

    fn req_with(qs: Vec<QuestionSummary>) -> ReportRequest {
        ReportRequest {
            survey_title: "테스트 설문".into(),
            persona_count: 100,
            persona_distribution: "여 50명 / 남 50명 · 평균 30세".into(),
            question_summaries: qs,
            style: ReportStyle::Mckinsey,
        }
    }

    #[test]
    fn estimate_tokens_korean_uses_25_per_char() {
        // "한국어" 3글자 → 7~8 토큰 추정.
        let t = estimate_tokens("한국어");
        assert!(t == 7);
    }

    #[test]
    fn single_chunk_when_under_limit() {
        let req = req_with(vec![sample_summary("q1", "짧은 질문이에요")]);
        let plan = build_plan_internal(&req, DEFAULT_CHUNK_TOKEN_LIMIT);
        assert_eq!(plan.chunks.len(), 1);
        assert_eq!(plan.chunks[0].seq, 1);
        assert_eq!(plan.chunks[0].total, 1);
        assert!(plan.final_synthesis.is_none(), "단일 청크는 합성 불필요");
        // 단일 청크 prompt에는 풀 style directive가 들어가야 함.
        assert!(plan.chunks[0].prompt.contains("McKinsey"));
    }

    #[test]
    fn multiple_chunks_when_over_limit() {
        // 작은 한도(500토큰)로 강제 분할 — 5개 질문이 분리되도록.
        let qs: Vec<_> = (0..5)
            .map(|i| sample_summary(&format!("q{i}"), "이것은 분할 테스트용 질문이에요. ".repeat(50).as_str()))
            .collect();
        let req = req_with(qs);
        let plan = build_plan_internal(&req, 500);
        assert!(plan.chunks.len() >= 2, "5개 큰 질문은 2+ 청크여야 함");
        // 모든 청크의 seq/total 정합.
        let total = plan.chunks.len();
        for (i, c) in plan.chunks.iter().enumerate() {
            assert_eq!(c.seq, i + 1);
            assert_eq!(c.total, total);
        }
        // 합성 prompt가 있어야 함.
        let synth = plan.final_synthesis.as_deref().expect("multi → synthesis");
        assert!(synth.contains("종합 합성"));
        assert!(synth.contains("McKinsey"));
    }

    #[test]
    fn each_question_kept_intact_in_one_chunk() {
        // 한 질문이 여러 청크로 쪼개지지 않음 (질문 단위 grouping).
        let qs: Vec<_> = (0..4)
            .map(|i| sample_summary(&format!("q{i}"), "내용"))
            .collect();
        let req = req_with(qs);
        let plan = build_plan_internal(&req, 100_000); // 큰 한도 → 1 chunk.
        assert_eq!(plan.chunks.len(), 1);
        // q0~q3 모두 단일 청크에 등장.
        for i in 0..4 {
            assert!(plan.chunks[0].prompt.contains(&format!("q{i}")));
        }
    }

    #[test]
    fn estimated_tokens_total_matches_sum_plus_synthesis() {
        let qs: Vec<_> = (0..3)
            .map(|i| sample_summary(&format!("q{i}"), "이것은 분할 테스트용 질문이에요. ".repeat(40).as_str()))
            .collect();
        let req = req_with(qs);
        let plan = build_plan_internal(&req, 500);
        let chunks_sum: u64 = plan.chunks.iter().map(|c| c.estimated_tokens).sum();
        let synth_tokens = plan
            .final_synthesis
            .as_deref()
            .map(estimate_tokens)
            .unwrap_or(0);
        assert_eq!(plan.estimated_tokens_total, chunks_sum + synth_tokens);
    }

    #[test]
    fn single_chunk_prompt_equivalent_to_full_prompt_skeleton() {
        // 회귀 가드 — chunks.len()==1일 때 사용자가 받는 prompt가 v0.8.3의 build_full_prompt와 동치 키워드 포함.
        let req = req_with(vec![sample_summary("q1", "테스트")]);
        let plan = build_plan_internal(&req, DEFAULT_CHUNK_TOKEN_LIMIT);
        let single = &plan.chunks[0].prompt;
        let full = build_full_prompt(&req);
        // 둘 다 핵심 토큰 포함.
        for marker in ["가상 페르소나", "테스트 설문", "가공 요청", "McKinsey"] {
            assert!(single.contains(marker), "single chunk missing {marker}");
            assert!(full.contains(marker), "full prompt missing {marker}");
        }
    }
}
