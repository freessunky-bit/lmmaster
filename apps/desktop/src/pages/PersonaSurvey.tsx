// 페르소나 시뮬레이션 — v0.8.x.
//
// v0.8.0: 데이터셋 자동 다운로드 + 진행률 GUI.
// v0.8.1+: 3-mode 페르소나 정의 / 설문 / 배치 실행 / 리포트.

import { useCallback, useEffect, useState } from "react";
import {
  CheckCircle2,
  Database,
  Download,
  Loader2,
  Sparkles,
  Users,
} from "lucide-react";

import {
  downloadPersonasDataset,
  getPersonasDatasetStatus,
  type PersonasDatasetEvent,
  type PersonasDatasetStatus,
} from "../ipc/personas";

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
  const [status, setStatus] = useState<PersonasDatasetStatus | null>(null);
  const [download, setDownload] = useState<DownloadState>({ kind: "idle" });
  const [statusError, setStatusError] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      const s = await getPersonasDatasetStatus();
      setStatus(s);
      setStatusError(null);
    } catch (e) {
      const msg = (e as { message?: string }).message ?? String(e);
      setStatusError(msg);
    }
  }, []);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const handleDownload = useCallback(async () => {
    setDownload({ kind: "starting" });
    try {
      await downloadPersonasDataset({
        onEvent: (event: PersonasDatasetEvent) => {
          setDownload((prev) => mergeEvent(prev, event));
        },
      });
      // download 완료 이벤트는 onEvent에서 처리됨. 여기서 status 다시 fetch.
      await refreshStatus();
    } catch (e) {
      const msg = (e as { message?: string }).message ?? String(e);
      setDownload({ kind: "failed", message: msg });
    }
  }, [refreshStatus]);

  const isDownloading =
    download.kind === "starting" || download.kind === "running";

  return (
    <div className="personas-root">
      <header className="personas-header">
        <div className="personas-title-row">
          <Users size={22} aria-hidden="true" className="personas-title-icon" />
          <h2 className="personas-title">가상 한국인 설문 시뮬레이션</h2>
          <span className="personas-badge">베타 v0.8.0</span>
        </div>
        <p className="personas-subtitle">
          엔비디아 <strong>Nemotron-Personas-Korea</strong> 데이터셋(700만 합성
          한국인)과 사용자 PC의 로컬 LLM으로, 마케팅 카피·UX 시안·콘텐츠를
          실제 사람에게 묻기 전에 가상으로 사전 검증해요.
        </p>
      </header>

      {/* 1단계: 데이터셋 자동 다운로드 — v0.8.0 핵심 */}
      <section className="personas-step" aria-labelledby="personas-step-1">
        <div className="personas-step-num">1</div>
        <div className="personas-step-body">
          <h3 id="personas-step-1" className="personas-step-title">
            <Database size={18} aria-hidden="true" /> 데이터셋 준비
          </h3>
          <p className="personas-step-desc">
            가상 한국인 700만 명의 인구통계 + 직업 + 성격 narrative가 담긴
            데이터셋이에요. 사용자가 입력한 조건으로 N명을 추출해서 시뮬레이션
            대상으로 써요. 한 번만 받으면 다음부터 즉시 사용 가능해요.
          </p>

          <DatasetCard
            status={status}
            statusError={statusError}
            download={download}
            isDownloading={isDownloading}
            onDownload={handleDownload}
          />
        </div>
      </section>

      {/* 2단계: 페르소나 정의 — v0.8.1 (잠금) */}
      <section className="personas-step is-locked" aria-labelledby="personas-step-2">
        <div className="personas-step-num">2</div>
        <div className="personas-step-body">
          <h3 id="personas-step-2" className="personas-step-title">
            <Users size={18} aria-hidden="true" /> 페르소나 정의
          </h3>
          <p className="personas-step-desc">
            텍스트, 문서, 또는 폼 3가지 방식으로 시뮬레이션 대상자를 정의해요.
            성별·연령·지역·직업·관심사를 자유롭게 셋업하고, AI가 데이터셋에서
            조건에 맞는 N명을 인구 분포 비례로 뽑아드려요.
          </p>
          <div className="personas-step-coming">
            <Loader2 size={14} className="personas-spin" aria-hidden="true" /> 다음 버전
            (v0.8.1)에서 도착해요
          </div>
        </div>
      </section>

      {/* 3단계: 설문 정의 + 실행 — v0.8.2 (잠금) */}
      <section className="personas-step is-locked" aria-labelledby="personas-step-3">
        <div className="personas-step-num">3</div>
        <div className="personas-step-body">
          <h3 id="personas-step-3" className="personas-step-title">
            <Sparkles size={18} aria-hidden="true" /> 설문 정의 + 실행
          </h3>
          <p className="personas-step-desc">
            설문지를 텍스트나 문서로 입력하면 AI가 객관식·척도·주관식을
            자동 분류해요. 정의된 N명에게 배치로 응답을 생성하고, 진행 상황을
            실시간으로 보여드려요.
          </p>
          <div className="personas-step-coming">
            <Loader2 size={14} className="personas-spin" aria-hidden="true" /> 다음 버전
            (v0.8.2)에서 도착해요
          </div>
        </div>
      </section>

      {/* 4단계: 결과 리포트 — v0.8.3 (잠금) */}
      <section className="personas-step is-locked" aria-labelledby="personas-step-4">
        <div className="personas-step-num">4</div>
        <div className="personas-step-body">
          <h3 id="personas-step-4" className="personas-step-title">
            <Sparkles size={18} aria-hidden="true" /> 결과 리포트
          </h3>
          <p className="personas-step-desc">
            응답을 자체 차트로 시각화하고, 외부 LLM(Gemini, ChatGPT, Claude
            등)에 바로 붙여 넣을 수 있는 <strong>전문 리서치 보고서 프롬프트</strong>를
            생성해요. McKinsey, 닐슨, 학술 논문 3가지 스타일을 지원해요.
          </p>
          <div className="personas-step-coming">
            <Loader2 size={14} className="personas-spin" aria-hidden="true" /> 다음 버전
            (v0.8.3)에서 도착해요
          </div>
        </div>
      </section>
    </div>
  );
}

// ── 데이터셋 카드 ─────────────────────────────────────────────────────

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
        <Loader2 size={16} className="personas-spin" aria-hidden="true" /> 상태
        확인 중…
      </div>
    );
  }

  if (status.installed && download.kind !== "running" && download.kind !== "starting") {
    return (
      <div className="personas-card is-installed" role="status">
        <div className="personas-card-row">
          <CheckCircle2
            size={18}
            aria-hidden="true"
            className="personas-card-icon"
          />
          <div className="personas-card-info">
            <strong>데이터셋 준비 완료</strong>
            <span className="personas-card-meta num">
              {status.file_count}개 파일 · {formatSize(status.size_bytes)}
            </span>
          </div>
          <button
            type="button"
            className="personas-btn-secondary"
            onClick={onDownload}
            disabled={isDownloading}
          >
            다시 받기
          </button>
        </div>
      </div>
    );
  }

  if (download.kind === "running" || download.kind === "starting") {
    const isRunning = download.kind === "running";
    const pct =
      isRunning && download.total > 0
        ? Math.round((download.completed / download.total) * 100)
        : null;
    return (
      <div className="personas-card is-downloading" role="status" aria-live="polite">
        <div className="personas-card-row">
          <Download
            size={18}
            aria-hidden="true"
            className="personas-card-icon personas-spin"
          />
          <div className="personas-card-info">
            <strong>
              {isRunning ? download.message : "다운로드 시작 중…"}
            </strong>
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
          <div
            className="personas-progress"
            role="progressbar"
            aria-valuenow={pct ?? 0}
            aria-valuemin={0}
            aria-valuemax={100}
          >
            <div
              className="personas-progress-bar"
              style={{ width: `${pct ?? 0}%` }}
            />
          </div>
        )}
      </div>
    );
  }

  if (download.kind === "failed") {
    return (
      <div className="personas-card is-error" role="alert">
        <p>
          <strong>다운로드 실패</strong> — {download.message}
        </p>
        <button
          type="button"
          className="personas-btn-primary"
          onClick={onDownload}
        >
          다시 시도할게요
        </button>
      </div>
    );
  }

  // idle / done & not installed (size mismatch 등)
  return (
    <div className="personas-card">
      <p className="personas-card-info-text">
        아직 받지 않았어요. 약 <strong>1.8 GB</strong>의 .parquet 파일을 받아요
        (CC BY 4.0). 한 번만 받으면 다음부턴 즉시 사용해요.
      </p>
      <button
        type="button"
        className="personas-btn-primary"
        onClick={onDownload}
        disabled={isDownloading}
        data-testid="personas-download"
      >
        <Download size={14} aria-hidden="true" /> 자동으로 받을게요
      </button>
    </div>
  );
}

// ── 이벤트 머지 ───────────────────────────────────────────────────────

function mergeEvent(
  prev: DownloadState,
  event: PersonasDatasetEvent,
): DownloadState {
  switch (event.kind) {
    case "status":
      return {
        kind: "running",
        message: event.status,
        fileIndex: event.file_index,
        fileTotal: event.file_total,
        completed:
          prev.kind === "running" && prev.fileIndex === event.file_index
            ? prev.completed
            : 0,
        total:
          prev.kind === "running" && prev.fileIndex === event.file_index
            ? prev.total
            : 0,
        speedBps: 0,
      };
    case "progress":
      if (prev.kind !== "running") {
        return {
          kind: "running",
          message: "받는 중",
          fileIndex: 0,
          fileTotal: 0,
          completed: event.completed_bytes,
          total: event.total_bytes,
          speedBps: event.speed_bps,
        };
      }
      return {
        ...prev,
        completed: event.completed_bytes,
        total: event.total_bytes,
        speedBps: event.speed_bps,
      };
    case "completed":
      return {
        kind: "done",
        fileCount: event.file_count,
        totalBytes: event.total_bytes,
      };
    case "failed":
      return { kind: "failed", message: event.message };
  }
}
