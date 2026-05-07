// InstalledDatasetsSection — Phase 23'.c.2.d.4.2.
//
// 정책:
// - listInstalledDatasets() 호출 + 카드 그리드 + 2-step 삭제 confirm.
// - 빈 상태 안내 (CLAUDE.md §4.1 — 해요체).
// - trends-card-* 디자인 토큰 재사용.
// - role="list"/"listitem" + aria-label 한국어.
// - 외부에서 onChanged() 받아 dataset import 완료 시 refetch trigger.

import { Database, RotateCw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  deleteInstalledDataset,
  listInstalledDatasets,
  sampleStrategyLabel,
  type InstalledDataset,
  type SampleStrategy,
} from "../../ipc/dataset-import";

export interface InstalledDatasetsSectionProps {
  /** dataset_import_start의 Done event 후 호출되어 자동 refetch 유발. */
  refreshSignal?: number;
}

interface DeleteState {
  /** null이면 어떤 카드도 confirm 단계 아님. */
  pendingId: string | null;
  /** 삭제 진행 중인 id. */
  deletingId: string | null;
  /** 삭제 실패 메시지. */
  error: string | null;
}

const INITIAL_DELETE: DeleteState = {
  pendingId: null,
  deletingId: null,
  error: null,
};

function parseSampleStrategy(json: string): SampleStrategy | null {
  try {
    const v = JSON.parse(json);
    if (
      v &&
      typeof v === "object" &&
      typeof v.kind === "string" &&
      ["full", "first", "stratified"].includes(v.kind)
    ) {
      return v as SampleStrategy;
    }
  } catch {
    /* fall through */
  }
  return null;
}

function formatDate(iso: string): string {
  try {
    const d = new Date(iso);
    if (!Number.isFinite(d.getTime())) return iso;
    return d.toLocaleDateString("ko-KR", {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
    });
  } catch {
    return iso;
  }
}

export function InstalledDatasetsSection({
  refreshSignal,
}: InstalledDatasetsSectionProps) {
  const { t } = useTranslation();
  const [datasets, setDatasets] = useState<InstalledDataset[]>([]);
  const [loading, setLoading] = useState(true);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const [deleteState, setDeleteState] = useState<DeleteState>(INITIAL_DELETE);

  const fetchDatasets = useCallback(async () => {
    setLoading(true);
    setFetchError(null);
    try {
      const list = await listInstalledDatasets();
      setDatasets(list);
    } catch (e) {
      setFetchError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchDatasets();
  }, [fetchDatasets, refreshSignal]);

  const handleDeleteClick = useCallback(
    (id: string) => {
      if (deleteState.pendingId === id) {
        // 2-step confirm — 두 번째 클릭 → 실 삭제.
        setDeleteState({ pendingId: null, deletingId: id, error: null });
        deleteInstalledDataset(id)
          .then(() => {
            setDeleteState(INITIAL_DELETE);
            void fetchDatasets();
          })
          .catch((e) => {
            setDeleteState({
              pendingId: null,
              deletingId: null,
              error: String(e),
            });
          });
      } else {
        setDeleteState({ pendingId: id, deletingId: null, error: null });
      }
    },
    [deleteState.pendingId, fetchDatasets],
  );

  const cancelDeleteConfirm = useCallback(() => {
    setDeleteState(INITIAL_DELETE);
  }, []);

  return (
    <section
      className="trends-section"
      aria-labelledby="installed-datasets-heading"
      data-testid="installed-datasets-section"
    >
      <h2
        id="installed-datasets-heading"
        className="trends-section-heading"
      >
        <Database size={18} aria-hidden="true" />
        <span>
          {t("installedDatasets.heading", "내 데이터셋")}
        </span>
        <button
          type="button"
          className="trends-section-refresh"
          onClick={() => void fetchDatasets()}
          disabled={loading}
          aria-label={t("installedDatasets.refreshAria", "목록 새로고침")}
        >
          <RotateCw size={14} aria-hidden="true" />
        </button>
      </h2>

      {loading && (
        <p className="trends-card-hint">
          {t("installedDatasets.loading", "목록 불러오고 있어요…")}
        </p>
      )}

      {!loading && fetchError && (
        <p className="trends-card-hint">
          {t(
            "installedDatasets.fetchError",
            "목록을 불러오지 못했어요: {{error}}",
            { error: fetchError },
          )}
        </p>
      )}

      {!loading && !fetchError && datasets.length === 0 && (
        <p className="trends-card-hint">
          {t(
            "installedDatasets.empty",
            "아직 가져온 데이터셋이 없어요. 위 카드에서 '이 데이터셋 가져올게요'를 눌러 보세요.",
          )}
        </p>
      )}

      {!loading && datasets.length > 0 && (
        <ul className="trends-grid" role="list">
          {datasets.map((ds) => {
            const sample = parseSampleStrategy(ds.sampleStrategy);
            const sampleLabel = sample
              ? sampleStrategyLabel(sample)
              : ds.sampleStrategy;
            const isPending = deleteState.pendingId === ds.id;
            const isDeleting = deleteState.deletingId === ds.id;
            return (
              <li
                key={ds.id}
                className="trends-card trends-card-status-available"
                role="listitem"
                data-testid={`installed-dataset-${ds.id}`}
              >
                <div className="trends-card-head">
                  <Database size={16} aria-hidden="true" />
                  <span className="trends-card-kind">
                    {ds.config} / {ds.split}
                  </span>
                </div>
                <h3 className="trends-card-title">{ds.repo}</h3>
                <p className="trends-card-hint">{sampleLabel}</p>
                <p className="trends-card-meta">
                  <span className="trends-card-meta-item">
                    {t("installedDatasets.licenseLabel", "라이선스")}:{" "}
                    {ds.license}
                  </span>
                  <span className="trends-card-meta-sep">·</span>
                  <span className="trends-card-meta-item">
                    {t("installedDatasets.chunksLabel", "청크")}:{" "}
                    {ds.totalChunks.toLocaleString()}
                  </span>
                  <span className="trends-card-meta-sep">·</span>
                  <span className="trends-card-meta-item">
                    {t("installedDatasets.dimLabel", "차원")}: {ds.embeddingDim}
                  </span>
                  <span className="trends-card-meta-sep">·</span>
                  <span className="trends-card-meta-item">
                    {formatDate(ds.createdAt)}
                  </span>
                </p>

                <div className="installed-dataset-actions">
                  {isPending ? (
                    <>
                      <span className="installed-dataset-confirm-text">
                        {t(
                          "installedDatasets.deleteConfirm",
                          "정말 삭제할까요? 임베딩 청크도 함께 사라져요.",
                        )}
                      </span>
                      <button
                        type="button"
                        className="trends-card-action installed-dataset-action-danger"
                        onClick={() => handleDeleteClick(ds.id)}
                        disabled={isDeleting}
                        aria-label={t(
                          "installedDatasets.deleteConfirmAria",
                          "{{repo}} 삭제 확인",
                          { repo: ds.repo },
                        )}
                      >
                        <Trash2
                          size={14}
                          aria-hidden="true"
                          style={{ marginRight: "var(--space-1)" }}
                        />
                        {t("installedDatasets.deleteConfirmCta", "정말 삭제할게요")}
                      </button>
                      <button
                        type="button"
                        className="trends-card-action"
                        onClick={cancelDeleteConfirm}
                        disabled={isDeleting}
                      >
                        {t("installedDatasets.cancelCta", "취소할래요")}
                      </button>
                    </>
                  ) : (
                    <button
                      type="button"
                      className="trends-card-action installed-dataset-action-danger"
                      onClick={() => handleDeleteClick(ds.id)}
                      disabled={isDeleting}
                      aria-label={t(
                        "installedDatasets.deleteAria",
                        "{{repo}} 삭제",
                        { repo: ds.repo },
                      )}
                    >
                      <Trash2
                        size={14}
                        aria-hidden="true"
                        style={{ marginRight: "var(--space-1)" }}
                      />
                      {t("installedDatasets.deleteCta", "삭제할래요")}
                    </button>
                  )}
                </div>

                {deleteState.error && deleteState.pendingId === null && (
                  <p
                    className="trends-card-hint"
                    style={{ color: "var(--danger, var(--text-muted))" }}
                  >
                    {t(
                      "installedDatasets.deleteFailed",
                      "삭제에 실패했어요: {{error}}",
                      { error: deleteState.error },
                    )}
                  </p>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
