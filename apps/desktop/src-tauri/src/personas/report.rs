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
    let mut out = String::new();
    out.push_str("# 가상 페르소나 설문 시뮬레이션 결과\n\n");
    out.push_str(&format!("- **설문**: {}\n", req.survey_title));
    out.push_str(&format!("- **응답자 수**: {} (가상 페르소나)\n", req.persona_count));
    out.push_str(&format!("- **분포**: {}\n", req.persona_distribution));
    out.push_str("- **출처**: NVIDIA Nemotron-Personas-Korea (CC BY 4.0) + 사용자 PC의 로컬 LLM\n\n");
    out.push_str("---\n\n## 설문 결과\n\n");
    for q in &req.question_summaries {
        out.push_str(&render_question(q));
    }
    out.push_str("---\n\n## 가공 요청\n\n");
    out.push_str(style_directive(req.style));
    out.push_str("\n\n출력은 마크다운으로, 한국어로, 추가 설명 없이 본문만 작성해 주세요.");
    Ok(out)
}
