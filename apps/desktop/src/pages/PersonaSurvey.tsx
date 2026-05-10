// 페르소나 시뮬레이션 — v0.8.x.
//
// v0.8.0: 데이터셋 자동 다운로드 + 진행률 GUI.
// v0.8.1: Step 2 페르소나 정의 (폼) + 샘플링 미리보기.
// v0.8.2: Step 3 설문 정의 (텍스트) + Step 4 배치 실행.
// v0.8.3: 결과 통계 + 외부 LLM 프롬프트 생성 (3종 스타일).
// v0.8.4: 추론 파라미터 GUI + chunked map-reduce 리포트 (토큰 한계 자동 분할).

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  AlertTriangle,
  BarChart3,
  CheckCircle2,
  ClipboardCopy,
  Database,
  Download,
  FileText,
  Loader2,
  Play,
  Sliders,
  Sparkles,
  Users,
} from "lucide-react";

import { version as APP_VERSION } from "../../package.json";

import { listLocalLlamaCppModels } from "../ipc/chat";
import { listRuntimeModels } from "../ipc/runtimes";
import {
  downloadPersonasDataset,
  getPersonasDatasetStatus,
  personasGenerateReportPromptPlan,
  personasRunSurvey,
  personasSample,
  type Persona,
  type PersonaFilter,
  type PersonasDatasetEvent,
  type PersonasDatasetStatus,
  type PersonasSurveyEvent,
  type ReportPromptPlan,
  type ReportStyle,
  type SurveyAnswer,
  type SurveyDef,
  type SurveyQuestion,
} from "../ipc/personas";
import {
  SamplingDrawer,
  effectiveSampling,
  loadPersistedSampling,
  type PersistedSampling,
} from "./persona-survey/SamplingDrawer";

import "./persona-survey.css";

// ── 진행 상태 ─────────────────────────────────────────────────────────

type DownloadState =
  | { kind: "idle" }
  | { kind: "starting" }
  | {
      kind: "running";
      message: string;
      fileIndex: number;
      fileTotal: number;
      completed: number;
      total: number;
      speedBps: number;
    }
  | { kind: "done"; fileCount: number; totalBytes: number }
  | { kind: "failed"; message: string };

function formatSize(bytes: number): string {
  if (!bytes) return "0";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

function formatSpeed(bps: number): string {
  if (!bps) return "—";
  return `${formatSize(bps)}/s`;
}

// ── 메인 페이지 ───────────────────────────────────────────────────────

export function PersonaSurveyPage() {
  // Step 1 — 데이터셋
  const [datasetStatus, setDatasetStatus] = useState<PersonasDatasetStatus | null>(null);
  const [download, setDownload] = useState<DownloadState>({ kind: "idle" });
  const [datasetError, setDatasetError] = useState<string | null>(null);

  // Step 2 — 페르소나
  const [personas, setPersonas] = useState<Persona[]>([]);

  // Step 3 — 설문
  const [survey, setSurvey] = useState<SurveyDef | null>(null);

  // Step 4 — 실행 결과 (reportPlan은 RunReportCard 내부에서 관리)
  const [answers, setAnswers] = useState<SurveyAnswer[]>([]);

  const refreshDatasetStatus = useCallback(async () => {
    try {
      const s = await getPersonasDatasetStatus();
      setDatasetStatus(s);
      setDatasetError(null);
    } catch (e) {
      setDatasetError((e as { message?: string }).message ?? String(e));
    }
  }, []);

  useEffect(() => {
    void refreshDatasetStatus();
  }, [refreshDatasetStatus]);

  const handleDownload = useCallback(async () => {
    setDownload({ kind: "starting" });
    try {
      await downloadPersonasDataset({
        onEvent: (event: PersonasDatasetEvent) => {
          setDownload((prev) => mergeDownloadEvent(prev, event));
        },
      });
      await refreshDatasetStatus();
    } catch (e) {
      setDownload({
        kind: "failed",
        message: (e as { message?: string }).message ?? String(e),
      });
    }
  }, [refreshDatasetStatus]);

  const isDownloading =
    download.kind === "starting" || download.kind === "running";
  const datasetReady = datasetStatus?.installed ?? false;

  return (
    <div className="personas-root">
      <header className="personas-header">
        <div className="personas-title-row">
          <Users size={22} aria-hidden="true" className="personas-title-icon" />
          <h2 className="personas-title">가상 한국인 설문 시뮬레이션</h2>
          <span className="personas-badge">베타 v{APP_VERSION}</span>
        </div>
        <p className="personas-subtitle">
          엔비디아 <strong>Nemotron-Personas-Korea</strong> 데이터셋에서 조건에 맞는
          가상 한국인 N명을 추출하고, 사용자 PC의 로컬 LLM으로 설문 응답을 시뮬레이션해요.
          마케팅·UX·콘텐츠를 실제 사람에게 묻기 전에 가상으로 사전 검증하세요.
        </p>
      </header>

      {/* Step 1: 데이터셋 */}
      <Step
        num={1}
        title="데이터셋 준비"
        icon={<Database size={18} aria-hidden="true" />}
        desc="가상 한국인 700만 명의 인구통계 + 직업 + 성격 narrative가 담긴 데이터셋이에요. 한 번만 받으면 다음부터 즉시 사용 가능해요."
        locked={false}
      >
        <DatasetCard
          status={datasetStatus}
          statusError={datasetError}
          download={download}
          isDownloading={isDownloading}
          onDownload={handleDownload}
        />
      </Step>

      {/* Step 2: 페르소나 정의 */}
      <Step
        num={2}
        title="페르소나 정의"
        icon={<Users size={18} aria-hidden="true" />}
        desc="시뮬레이션 대상자의 성별·연령·지역·관심사를 정의하면 AI가 인구 분포에 맞춰 N명을 추출해요."
        locked={!datasetReady}
        lockedReason="먼저 1단계에서 데이터셋을 받아주세요"
      >
        {datasetReady && (
          <PersonasDefineCard onSampled={(p) => setPersonas(p)} personas={personas} />
        )}
      </Step>

      {/* Step 3: 설문 정의 */}
      <Step
        num={3}
        title="설문 정의"
        icon={<FileText size={18} aria-hidden="true" />}
        desc="물어볼 질문을 입력해요. 객관식·척도·주관식을 섞어 쓸 수 있어요."
        locked={personas.length === 0}
        lockedReason="먼저 2단계에서 페르소나를 추출해 주세요"
      >
        {personas.length > 0 && (
          <SurveyDefineCard onDefined={(s) => setSurvey(s)} survey={survey} />
        )}
      </Step>

      {/* Step 4: 실행 + 리포트 */}
      <Step
        num={4}
        title="배치 실행 + 리포트"
        icon={<Sparkles size={18} aria-hidden="true" />}
        desc="N명에게 M문항을 배치로 묻고, 결과를 차트로 보여드리거나 외부 LLM에 붙여 넣을 수 있는 리서치 리포트 프롬프트를 만들어요."
        locked={!survey}
        lockedReason="먼저 3단계에서 설문을 정의해 주세요"
      >
        {survey && (
          <RunReportCard
            personas={personas}
            survey={survey}
            answers={answers}
            setAnswers={setAnswers}
          />
        )}
      </Step>
    </div>
  );
}

// ── 공통 Step Wrapper ────────────────────────────────────────────────

function Step({
  num,
  title,
  icon,
  desc,
  locked,
  lockedReason,
  children,
}: {
  num: number;
  title: string;
  icon: React.ReactNode;
  desc: string;
  locked: boolean;
  lockedReason?: string;
  children?: React.ReactNode;
}) {
  return (
    <section className={`personas-step${locked ? " is-locked" : ""}`}>
      <div className="personas-step-num">{num}</div>
      <div className="personas-step-body">
        <h3 className="personas-step-title">
          {icon} {title}
        </h3>
        <p className="personas-step-desc">{desc}</p>
        {locked ? (
          <div className="personas-step-coming">{lockedReason ?? "잠금"}</div>
        ) : (
          children
        )}
      </div>
    </section>
  );
}

// ── Step 1 — 데이터셋 카드 (v0.8.0 그대로) ───────────────────────────

function DatasetCard({
  status,
  statusError,
  download,
  isDownloading,
  onDownload,
}: {
  status: PersonasDatasetStatus | null;
  statusError: string | null;
  download: DownloadState;
  isDownloading: boolean;
  onDownload: () => void;
}) {
  if (statusError) {
    return (
      <div className="personas-card is-error" role="alert">
        <p>데이터셋 상태를 확인하지 못했어요: {statusError}</p>
      </div>
    );
  }
  if (status === null) {
    return (
      <div className="personas-card is-loading">
        <Loader2 size={16} className="personas-spin" aria-hidden="true" /> 상태 확인 중…
      </div>
    );
  }
  if (status.installed && !isDownloading) {
    return (
      <div className="personas-card is-installed" role="status">
        <div className="personas-card-row">
          <CheckCircle2 size={18} aria-hidden="true" className="personas-card-icon" />
          <div className="personas-card-info">
            <strong>데이터셋 준비 완료</strong>
            <span className="personas-card-meta num">
              {status.file_count}개 파일 · {formatSize(status.size_bytes)}
            </span>
          </div>
          <button type="button" className="personas-btn-secondary" onClick={onDownload} disabled={isDownloading}>
            다시 받기
          </button>
        </div>
      </div>
    );
  }
  if (download.kind === "running" || download.kind === "starting") {
    const isRunning = download.kind === "running";
    const pct = isRunning && download.total > 0
      ? Math.round((download.completed / download.total) * 100) : null;
    return (
      <div className="personas-card is-downloading" role="status" aria-live="polite">
        <div className="personas-card-row">
          <Download size={18} aria-hidden="true" className="personas-card-icon personas-spin" />
          <div className="personas-card-info">
            <strong>{isRunning ? download.message : "다운로드 시작 중…"}</strong>
            {isRunning && (
              <span className="personas-card-meta num">
                파일 {download.fileIndex} / {download.fileTotal}
                {pct !== null ? ` · ${pct}%` : ""}
                {download.speedBps > 0 ? ` · ${formatSpeed(download.speedBps)}` : ""}
              </span>
            )}
          </div>
        </div>
        {isRunning && download.total > 0 && (
          <div className="personas-progress" role="progressbar" aria-valuenow={pct ?? 0} aria-valuemin={0} aria-valuemax={100}>
            <div className="personas-progress-bar" style={{ width: `${pct ?? 0}%` }} />
          </div>
        )}
      </div>
    );
  }
  if (download.kind === "failed") {
    return (
      <div className="personas-card is-error" role="alert">
        <p><strong>다운로드 실패</strong> — {download.message}</p>
        <button type="button" className="personas-btn-primary" onClick={onDownload}>다시 시도할게요</button>
      </div>
    );
  }
  return (
    <div className="personas-card">
      <p className="personas-card-info-text">
        아직 받지 않았어요. 약 <strong>1.8 GB</strong>의 .parquet 파일을 받아요 (CC BY 4.0).
        한 번만 받으면 다음부턴 즉시 사용해요.
      </p>
      <button type="button" className="personas-btn-primary" onClick={onDownload} disabled={isDownloading}>
        <Download size={14} aria-hidden="true" /> 자동으로 받을게요
      </button>
    </div>
  );
}

function mergeDownloadEvent(prev: DownloadState, event: PersonasDatasetEvent): DownloadState {
  switch (event.kind) {
    case "status":
      return {
        kind: "running",
        message: event.status,
        fileIndex: event.file_index,
        fileTotal: event.file_total,
        completed: prev.kind === "running" && prev.fileIndex === event.file_index ? prev.completed : 0,
        total: prev.kind === "running" && prev.fileIndex === event.file_index ? prev.total : 0,
        speedBps: 0,
      };
    case "progress":
      if (prev.kind !== "running") {
        return { kind: "running", message: "받는 중", fileIndex: 0, fileTotal: 0,
          completed: event.completed_bytes, total: event.total_bytes, speedBps: event.speed_bps };
      }
      return { ...prev, completed: event.completed_bytes, total: event.total_bytes, speedBps: event.speed_bps };
    case "completed":
      return { kind: "done", fileCount: event.file_count, totalBytes: event.total_bytes };
    case "failed":
      return { kind: "failed", message: event.message };
  }
}

// ── Step 2 — 페르소나 정의 카드 (v0.8.1) ─────────────────────────────

function PersonasDefineCard({
  onSampled,
  personas,
}: {
  onSampled: (personas: Persona[]) => void;
  personas: Persona[];
}) {
  const [sex, setSex] = useState<string>("");
  const [ageMin, setAgeMin] = useState<number>(20);
  const [ageMax, setAgeMax] = useState<number>(59);
  const [provinces, setProvinces] = useState<string>("");
  const [occupations, setOccupations] = useState<string>("");
  const [keywords, setKeywords] = useState<string>("");
  const [sampleSize, setSampleSize] = useState<number>(20);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSample = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const filter: PersonaFilter = {
        sex: sex || null,
        age_min: ageMin || null,
        age_max: ageMax || null,
        province_includes: provinces.split(/[,\s]+/).filter(Boolean),
        occupation_includes: occupations.split(/[,\s]+/).filter(Boolean),
        keyword_includes: keywords.split(/[,\s]+/).filter(Boolean),
        sample_size: sampleSize,
      };
      const result = await personasSample(filter);
      onSampled(result);
    } catch (e) {
      setError((e as { message?: string }).message ?? String(e));
    } finally {
      setBusy(false);
    }
  }, [sex, ageMin, ageMax, provinces, occupations, keywords, sampleSize, onSampled]);

  // 분포 요약.
  const distribution = useMemo(() => {
    if (personas.length === 0) return null;
    const sexCounts: Record<string, number> = {};
    let ageSum = 0;
    let ageN = 0;
    for (const p of personas) {
      sexCounts[p.sex || "?"] = (sexCounts[p.sex || "?"] ?? 0) + 1;
      const age = parseInt(p.age, 10);
      if (!isNaN(age)) {
        ageSum += age;
        ageN++;
      }
    }
    const sexPart = Object.entries(sexCounts)
      .map(([k, v]) => `${k} ${v}`)
      .join(" / ");
    const avgAge = ageN > 0 ? (ageSum / ageN).toFixed(1) : "?";
    return `${sexPart} · 평균 ${avgAge}세`;
  }, [personas]);

  return (
    <div className="personas-card">
      <div className="personas-form-grid">
        <label className="personas-form-row">
          <span>성별</span>
          <select value={sex} onChange={(e) => setSex(e.target.value)} disabled={busy}>
            <option value="">전체</option>
            <option value="F">여성</option>
            <option value="M">남성</option>
          </select>
        </label>
        <label className="personas-form-row">
          <span>연령 범위</span>
          <span className="personas-form-inline">
            <input type="number" min={0} max={120} value={ageMin}
              onChange={(e) => setAgeMin(parseInt(e.target.value || "0", 10))} disabled={busy} />
            <span> ~ </span>
            <input type="number" min={0} max={120} value={ageMax}
              onChange={(e) => setAgeMax(parseInt(e.target.value || "0", 10))} disabled={busy} />
            <span>세</span>
          </span>
        </label>
        <label className="personas-form-row">
          <span>지역 키워드</span>
          <input type="text" value={provinces}
            onChange={(e) => setProvinces(e.target.value)} placeholder="서울, 경기"
            disabled={busy} />
        </label>
        <label className="personas-form-row">
          <span>직업 키워드</span>
          <input type="text" value={occupations}
            onChange={(e) => setOccupations(e.target.value)} placeholder="직장인, 학생"
            disabled={busy} />
        </label>
        <label className="personas-form-row">
          <span>관심사 키워드</span>
          <input type="text" value={keywords}
            onChange={(e) => setKeywords(e.target.value)} placeholder="패션, 게임, 영화 (페르소나 텍스트 매치)"
            disabled={busy} />
        </label>
        <label className="personas-form-row">
          <span>인원 수</span>
          <input type="number" min={1} max={500} value={sampleSize}
            onChange={(e) => setSampleSize(parseInt(e.target.value || "1", 10))} disabled={busy} />
        </label>
      </div>

      {error && <p className="personas-card-info-text" style={{ color: "var(--error, #f87171)" }}>{error}</p>}

      <div className="personas-form-actions">
        <button type="button" className="personas-btn-primary" onClick={handleSample} disabled={busy}>
          {busy ? <Loader2 size={14} className="personas-spin" /> : <Users size={14} />}
          {busy ? "추출 중…" : "조건에 맞는 페르소나 추출"}
        </button>
      </div>

      {personas.length > 0 && (
        <div className="personas-card is-installed" role="status">
          <div className="personas-card-row">
            <CheckCircle2 size={18} aria-hidden="true" className="personas-card-icon" />
            <div className="personas-card-info">
              <strong>{personas.length}명 추출 완료</strong>
              <span className="personas-card-meta">{distribution}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ── Step 3 — 설문 정의 카드 (v0.8.2) ─────────────────────────────────

function SurveyDefineCard({
  onDefined,
  survey,
}: {
  onDefined: (s: SurveyDef) => void;
  survey: SurveyDef | null;
}) {
  const [text, setText] = useState<string>(SURVEY_TEMPLATE);
  const [error, setError] = useState<string | null>(null);

  const handleParse = useCallback(() => {
    setError(null);
    try {
      const parsed = parseSurveyText(text);
      if (parsed.questions.length === 0) {
        setError("질문을 1개 이상 정의해 주세요.");
        return;
      }
      onDefined(parsed);
    } catch (e) {
      setError((e as Error).message);
    }
  }, [text, onDefined]);

  return (
    <div className="personas-card">
      <p className="personas-card-info-text">
        한 줄에 질문 하나, 그 다음 줄에 보기를 적어요. 형식 예시:
      </p>
      <textarea
        className="personas-textarea"
        value={text}
        onChange={(e) => setText(e.target.value)}
        rows={12}
        spellCheck={false}
      />
      {error && (
        <p className="personas-card-info-text" style={{ color: "var(--error, #f87171)" }}>
          {error}
        </p>
      )}
      <div className="personas-form-actions">
        <button type="button" className="personas-btn-primary" onClick={handleParse}>
          <FileText size={14} /> 설문 확정
        </button>
      </div>
      {survey && (
        <div className="personas-card is-installed" role="status">
          <div className="personas-card-row">
            <CheckCircle2 size={18} aria-hidden="true" className="personas-card-icon" />
            <div className="personas-card-info">
              <strong>{survey.questions.length}개 질문 등록</strong>
              <span className="personas-card-meta">{survey.title}</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

const SURVEY_TEMPLATE = `# 콘텐츠 선호도 사전 조사

[single] 주말 저녁 60분이 비었어요. 어떤 콘텐츠를 가장 보고 싶으세요?
- 드라마 1편
- 예능 1편
- 영화 1편
- 유튜브 짧은 영상 여러 개

[scale] 최근 한 달 OTT 사용 빈도는? (1=전혀 안 씀, 5=거의 매일)

[open] 최근 가장 인상 깊게 본 한국 콘텐츠 한 작품과 그 이유는?
`;

function parseSurveyText(text: string): SurveyDef {
  const lines = text.split(/\r?\n/);
  let title = "사용자 정의 설문";
  const questions: SurveyQuestion[] = [];
  let current: SurveyQuestion | null = null;
  let qIndex = 0;
  for (const raw of lines) {
    const line = raw.trim();
    if (!line) continue;
    if (line.startsWith("# ")) {
      title = line.slice(2).trim();
      continue;
    }
    const typeMatch = line.match(/^\[(single|multi|scale|open)\]\s*(.*)/i);
    if (typeMatch) {
      if (current) questions.push(current);
      qIndex++;
      const t = typeMatch[1]!.toLowerCase() as SurveyQuestion["type"];
      current = {
        id: `q${qIndex}`,
        type: t,
        text: typeMatch[2]!.trim(),
        options: t === "single" || t === "multi" ? [] : undefined,
      };
      continue;
    }
    if (current && (line.startsWith("- ") || line.startsWith("* "))) {
      if (current.options) {
        current.options.push(line.slice(2).trim());
      }
      continue;
    }
    // 자유 라인 — 직전 질문에 이어붙이기.
    if (current) {
      current.text = `${current.text}\n${line}`;
    }
  }
  if (current) questions.push(current);
  return {
    survey_id: `survey-${Date.now()}`,
    title,
    questions,
  };
}

// ── Step 4 — 실행 + 리포트 카드 (v0.8.2 + v0.8.3) ────────────────────

interface RunModelChoice {
  runtimeKind: "ollama" | "llama-cpp";
  modelId: string;
  displayName: string;
}

function RunReportCard({
  personas,
  survey,
  answers,
  setAnswers,
}: {
  personas: Persona[];
  survey: SurveyDef;
  answers: SurveyAnswer[];
  setAnswers: (a: SurveyAnswer[]) => void;
}) {
  const [models, setModels] = useState<RunModelChoice[]>([]);
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [running, setRunning] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number; current: string } | null>(null);
  const [systemExtra, setSystemExtra] = useState<string>("");
  const [style, setStyle] = useState<ReportStyle>("mckinsey");
  const [error, setError] = useState<string | null>(null);
  // v0.8.4 — 리포트 프롬프트 plan + 청크 복사 진행률.
  const [reportPlan, setReportPlan] = useState<ReportPromptPlan | null>(null);
  const [copiedChunks, setCopiedChunks] = useState<Set<number>>(new Set());
  const [synthCopied, setSynthCopied] = useState(false);
  // v0.8.4 — sampling Drawer state.
  const [sampling, setSampling] = useState<PersistedSampling>(() => loadPersistedSampling());
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [drawerFocusField, setDrawerFocusField] = useState<"max_tokens" | null>(null);

  // 사용 가능 모델 로드 (Ollama + llama-cpp).
  useEffect(() => {
    void (async () => {
      const all: RunModelChoice[] = [];
      try {
        const ollama = await listRuntimeModels("ollama");
        for (const m of ollama) {
          all.push({ runtimeKind: "ollama", modelId: m.id, displayName: m.id });
        }
      } catch { /* noop */ }
      try {
        const llama = await listLocalLlamaCppModels();
        for (const id of llama) {
          all.push({ runtimeKind: "llama-cpp", modelId: id, displayName: `[llama-cpp] ${id}` });
        }
      } catch { /* noop */ }
      setModels(all);
      if (all.length > 0) setSelectedModel(`${all[0]!.runtimeKind}::${all[0]!.modelId}`);
    })();
  }, []);

  const handleRun = useCallback(async () => {
    setRunning(true);
    setError(null);
    setAnswers([]);
    setProgress({ done: 0, total: personas.length * survey.questions.length, current: "준비 중" });
    const collected: SurveyAnswer[] = [];
    try {
      const [rk, mid] = selectedModel.split("::");
      await personasRunSurvey({
        personas,
        survey,
        runtimeKind: rk as "ollama" | "llama-cpp",
        modelId: mid!,
        systemExtra: systemExtra.trim() || undefined,
        sampling: effectiveSampling(sampling),
        onEvent: (e: PersonasSurveyEvent) => {
          if (e.kind === "answer") {
            collected.push(e.answer);
            setAnswers([...collected]);
          } else if (e.kind === "progress") {
            setProgress({ done: e.completed, total: e.total, current: `${e.current_persona.slice(0, 8)} · ${e.current_question}` });
          } else if (e.kind === "failed") {
            setError(e.message);
          }
        },
      });
    } catch (e) {
      setError((e as { message?: string }).message ?? String(e));
    } finally {
      setRunning(false);
      setProgress(null);
    }
  }, [personas, survey, selectedModel, systemExtra, sampling, setAnswers]);

  // v0.8.4 — 통계 집계 + chunked 리포트 plan 생성.
  const handleGenerateReport = useCallback(async () => {
    setError(null);
    setCopiedChunks(new Set());
    setSynthCopied(false);
    try {
      const summaries = aggregateAnswers(survey, answers);
      const distribution = describeDistribution(personas);
      const plan = await personasGenerateReportPromptPlan({
        survey_title: survey.title,
        persona_count: personas.length,
        persona_distribution: distribution,
        question_summaries: summaries,
        style,
      });
      setReportPlan(plan);
    } catch (e) {
      setError((e as { message?: string }).message ?? String(e));
    }
  }, [survey, answers, personas, style]);

  const handleCopyChunk = useCallback(async (seq: number, prompt: string) => {
    try {
      await navigator.clipboard.writeText(prompt);
      setCopiedChunks((prev) => new Set(prev).add(seq));
      setTimeout(() => {
        setCopiedChunks((prev) => {
          const next = new Set(prev);
          next.delete(seq);
          return next;
        });
      }, 2000);
    } catch { /* noop */ }
  }, []);

  const handleCopySynth = useCallback(async (synth: string) => {
    try {
      await navigator.clipboard.writeText(synth);
      setSynthCopied(true);
      setTimeout(() => setSynthCopied(false), 2000);
    } catch { /* noop */ }
  }, []);

  // v0.8.4 — 잘림 칩 클릭 → drawer 열고 max_tokens auto-focus.
  const openDrawerForTruncation = useCallback(() => {
    setDrawerFocusField("max_tokens");
    setDrawerOpen(true);
  }, []);
  const openDrawer = useCallback(() => {
    setDrawerFocusField(null);
    setDrawerOpen(true);
  }, []);

  const truncatedCount = useMemo(
    () => answers.filter((a) => a.truncated).length,
    [answers],
  );

  const handleDownloadCsv = useCallback(() => {
    const csv = answersToCsv(answers, personas, survey);
    const blob = new Blob([csv], { type: "text/csv;charset=utf-8;" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `survey-results-${Date.now()}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }, [answers, personas, survey]);

  const totalCalls = personas.length * survey.questions.length;

  return (
    <div className="personas-card">
      <div className="personas-form-grid">
        <label className="personas-form-row">
          <span>모델</span>
          <select value={selectedModel} onChange={(e) => setSelectedModel(e.target.value)} disabled={running || models.length === 0}>
            {models.length === 0 && <option value="">받은 모델이 없어요</option>}
            {models.map((m) => (
              <option key={`${m.runtimeKind}::${m.modelId}`} value={`${m.runtimeKind}::${m.modelId}`}>
                {m.displayName}
              </option>
            ))}
          </select>
        </label>
        <label className="personas-form-row">
          <span>추가 system 지시</span>
          <input type="text" value={systemExtra} onChange={(e) => setSystemExtra(e.target.value)}
            placeholder="(선택) 반드시 한국어로만, 한 줄로 답해 주세요" disabled={running} />
        </label>
      </div>

      <p className="personas-card-info-text">
        총 호출: <strong>{personas.length}명 × {survey.questions.length}문항 = {totalCalls}회</strong>{" "}
        — 4B 모델 기준 약 {Math.ceil(totalCalls * 3 / 60)}분 소요 (PC 사양에 따라 다름).
      </p>

      {error && (
        <p className="personas-card-info-text" style={{ color: "var(--error, #f87171)" }}>{error}</p>
      )}

      <div className="personas-form-actions">
        <button type="button" className="personas-btn-primary" onClick={handleRun} disabled={running || !selectedModel}>
          {running ? <Loader2 size={14} className="personas-spin" /> : <Play size={14} />}
          {running ? "응답 생성 중…" : "배치 시작"}
        </button>
        <button
          type="button"
          className="personas-btn-secondary"
          onClick={openDrawer}
          disabled={running}
          title="추론 파라미터 (max_tokens, temperature, top_p, ...)"
        >
          <Sliders size={14} aria-hidden="true" /> 추론 설정 ({samplingPresetLabel(sampling)})
        </button>
      </div>

      {progress && (
        <div className="personas-card is-downloading" role="status" aria-live="polite">
          <div className="personas-card-row">
            <Sparkles size={18} className="personas-spin personas-card-icon" />
            <div className="personas-card-info">
              <strong>{progress.done} / {progress.total}</strong>
              <span className="personas-card-meta num">현재: {progress.current}</span>
            </div>
          </div>
          <div className="personas-progress" role="progressbar" aria-valuenow={progress.done} aria-valuemin={0} aria-valuemax={progress.total}>
            <div className="personas-progress-bar" style={{ width: `${progress.total > 0 ? (progress.done / progress.total) * 100 : 0}%` }} />
          </div>
        </div>
      )}

      {answers.length > 0 && !running && (
        <>
          <div className="personas-card is-installed" role="status">
            <div className="personas-card-row">
              <CheckCircle2 size={18} className="personas-card-icon" />
              <div className="personas-card-info">
                <strong>{answers.length}건 응답 수집 완료</strong>
                <span className="personas-card-meta">결과를 차트나 LLM 프롬프트로 가공해 보세요</span>
              </div>
            </div>
            {truncatedCount > 0 && (
              <button
                type="button"
                className="personas-truncated-chip"
                onClick={openDrawerForTruncation}
                title="잘린 응답이 있어요 — 최대 토큰을 늘려서 다시 받아볼래요?"
              >
                <AlertTriangle size={14} aria-hidden="true" />
                <span>
                  <strong>{truncatedCount}건</strong>이 토큰 한도로 잘렸어요 ·{" "}
                  <em>최대 답변 길이 늘려서 다시 받아볼래요?</em>
                </span>
              </button>
            )}
          </div>

          <div className="personas-form-grid">
            <label className="personas-form-row">
              <span>리포트 스타일</span>
              <select value={style} onChange={(e) => setStyle(e.target.value as ReportStyle)}>
                <option value="mckinsey">맥킨지(McKinsey) 컨설팅 보고서</option>
                <option value="nielsen">닐슨(Nielsen) UX 리서치</option>
                <option value="academic">학술 논문</option>
              </select>
            </label>
          </div>

          <div className="personas-form-actions">
            <button type="button" className="personas-btn-primary" onClick={handleGenerateReport}>
              <BarChart3 size={14} /> 리포트 프롬프트 생성
            </button>
            <button type="button" className="personas-btn-secondary" onClick={handleDownloadCsv}>
              CSV 다운로드
            </button>
          </div>

          {reportPlan && (
            <ChunkedReportSection
              plan={reportPlan}
              copiedChunks={copiedChunks}
              synthCopied={synthCopied}
              onCopyChunk={handleCopyChunk}
              onCopySynth={handleCopySynth}
            />
          )}
        </>
      )}

      <SamplingDrawer
        open={drawerOpen}
        initial={sampling}
        focusField={drawerFocusField}
        onClose={() => setDrawerOpen(false)}
        onApply={(next) => setSampling(next)}
      />
    </div>
  );
}

// ── ChunkedReportSection (v0.8.4) ────────────────────────────────────

function ChunkedReportSection({
  plan,
  copiedChunks,
  synthCopied,
  onCopyChunk,
  onCopySynth,
}: {
  plan: ReportPromptPlan;
  copiedChunks: Set<number>;
  synthCopied: boolean;
  onCopyChunk: (seq: number, prompt: string) => void;
  onCopySynth: (synth: string) => void;
}) {
  const isMulti = plan.chunks.length > 1;
  return (
    <div className="personas-prompt-area">
      <div
        className="personas-form-actions"
        style={{ justifyContent: "space-between" }}
      >
        <strong>
          외부 LLM(ChatGPT/Gemini/Claude)에 붙여넣을 프롬프트
          {isMulti && (
            <span className="personas-card-meta num">
              {" "}
              · {plan.chunks.length}개 청크 + 종합 1개 · 약{" "}
              {plan.estimated_tokens_total.toLocaleString()} 토큰
            </span>
          )}
        </strong>
      </div>
      {isMulti && (
        <p className="personas-card-info-text">
          데이터가 커서 외부 LLM 한 번에 못 보내요. <strong>{plan.chunks.length}개 청크를 순서대로</strong>{" "}
          복사해서 같은 대화창에 붙여 넣은 뒤, 마지막에 <strong>종합 합성</strong>{" "}
          프롬프트를 붙여 주세요. 외부 LLM이 모든 청크를 종합해 한 편의 리포트로 작성해요.
        </p>
      )}
      {plan.chunks.map((chunk) => (
        <div key={chunk.seq} className="personas-chunk-card">
          <div className="personas-form-actions" style={{ justifyContent: "space-between" }}>
            <strong>
              청크 {chunk.seq} / {chunk.total} ·{" "}
              <span className="personas-card-meta num">
                약 {chunk.estimated_tokens.toLocaleString()} 토큰
              </span>
            </strong>
            <button
              type="button"
              className="personas-btn-primary"
              onClick={() => onCopyChunk(chunk.seq, chunk.prompt)}
            >
              <ClipboardCopy size={14} aria-hidden="true" />{" "}
              {copiedChunks.has(chunk.seq) ? "복사 완료!" : "이 청크 복사"}
            </button>
          </div>
          <textarea
            className="personas-textarea"
            value={chunk.prompt}
            readOnly
            rows={isMulti ? 8 : 14}
          />
        </div>
      ))}
      {plan.final_synthesis && (
        <div className="personas-chunk-card personas-chunk-synth">
          <div className="personas-form-actions" style={{ justifyContent: "space-between" }}>
            <strong>종합 합성 프롬프트 (모든 청크를 보낸 뒤 마지막에)</strong>
            <button
              type="button"
              className="personas-btn-primary"
              onClick={() => onCopySynth(plan.final_synthesis!)}
            >
              <ClipboardCopy size={14} aria-hidden="true" />{" "}
              {synthCopied ? "복사 완료!" : "합성 프롬프트 복사"}
            </button>
          </div>
          <textarea
            className="personas-textarea"
            value={plan.final_synthesis}
            readOnly
            rows={8}
          />
        </div>
      )}
    </div>
  );
}

// ── sampling 프리셋 라벨 ─────────────────────────────────────────────

function samplingPresetLabel(p: PersistedSampling): string {
  switch (p.preset) {
    case "precise":
      return "정확하게";
    case "balanced":
      return "균형";
    case "creative":
      return "창의적";
    case "custom":
      return "직접";
  }
}

// ── 통계 집계 헬퍼 ───────────────────────────────────────────────────

function describeDistribution(personas: Persona[]): string {
  if (personas.length === 0) return "응답자 0명";
  const sex: Record<string, number> = {};
  let ageSum = 0, ageN = 0;
  for (const p of personas) {
    sex[p.sex || "?"] = (sex[p.sex || "?"] ?? 0) + 1;
    const a = parseInt(p.age, 10);
    if (!isNaN(a)) { ageSum += a; ageN++; }
  }
  const sexPart = Object.entries(sex).map(([k, v]) => `${k} ${v}명`).join(" / ");
  const avgAge = ageN > 0 ? (ageSum / ageN).toFixed(1) : "?";
  return `${sexPart} · 평균 ${avgAge}세 · 총 ${personas.length}명`;
}

function aggregateAnswers(survey: SurveyDef, answers: SurveyAnswer[]) {
  return survey.questions.map((q) => {
    const qAnswers = answers.filter((a) => a.question_id === q.id);
    if (q.type === "single" || q.type === "multi") {
      const counts: Record<string, number> = {};
      for (const a of qAnswers) {
        // 보기 텍스트 부분 매칭 — 가장 길게 일치하는 보기 선택.
        let matched: string | null = null;
        for (const opt of q.options ?? []) {
          if (a.answer.includes(opt)) {
            if (!matched || opt.length > matched.length) matched = opt;
          }
        }
        const key = matched ?? a.answer.slice(0, 30);
        counts[key] = (counts[key] ?? 0) + 1;
      }
      return {
        id: q.id, text: q.text, type: q.type,
        option_counts: Object.entries(counts).map(([option, count]) => ({ option, count })),
      };
    }
    if (q.type === "scale") {
      let sum = 0, n = 0;
      for (const a of qAnswers) {
        const m = a.answer.match(/[1-9]/);
        if (m) {
          const v = parseInt(m[0], 10);
          if (!isNaN(v)) { sum += v; n++; }
        }
      }
      return {
        id: q.id, text: q.text, type: q.type,
        scale_mean: n > 0 ? sum / n : null,
      };
    }
    // open
    const samples = qAnswers.slice(0, 5).map((a) => a.answer);
    const freq: Record<string, number> = {};
    for (const a of qAnswers) {
      const tokens = a.answer.split(/[\s,.!?\n]+/).filter((t) => t.length >= 2 && t.length <= 12);
      for (const t of tokens) freq[t] = (freq[t] ?? 0) + 1;
    }
    const top = Object.entries(freq).sort((a, b) => b[1] - a[1]).slice(0, 10)
      .map(([keyword, count]) => ({ keyword, count }));
    return {
      id: q.id, text: q.text, type: q.type,
      open_samples: samples, open_keyword_freq: top,
    };
  });
}

function answersToCsv(answers: SurveyAnswer[], personas: Persona[], survey: SurveyDef): string {
  const personaMap = new Map(personas.map((p) => [p.uuid, p]));
  const header = ["persona_uuid", "sex", "age", "province", "occupation", "question_id", "question_text", "answer", "took_ms"].join(",");
  const rows = answers.map((a) => {
    const p = personaMap.get(a.persona_uuid);
    const q = survey.questions.find((q) => q.id === a.question_id);
    return [
      a.persona_uuid,
      p?.sex ?? "",
      p?.age ?? "",
      `"${(p?.province ?? "").replace(/"/g, '""')}"`,
      `"${(p?.occupation ?? "").replace(/"/g, '""')}"`,
      a.question_id,
      `"${(q?.text ?? "").replace(/"/g, '""').replace(/\n/g, " ")}"`,
      `"${a.answer.replace(/"/g, '""').replace(/\n/g, " ")}"`,
      a.took_ms.toString(),
    ].join(",");
  });
  return [header, ...rows].join("\n");
}
