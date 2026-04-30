// PromptTemplateStep — Phase 12'.a (ADR-0050) Stage 0: 즉시 사용 프롬프트 템플릿.
//
// 정책:
// - URL hash로 들어온 모델의 `use_case_examples`를 카드 그리드로 노출.
// - 클릭 시 navigator.clipboard.writeText + 한국어 toast.
// - "내 패턴 저장" — localStorage (`lmmaster.prompts.<intent>` key).
//   → 파일 IPC는 v2+ deferred. localStorage가 외부 통신 0 + 작은 변경 면적.
// - URL hash 없이 들어오면 본 컴포넌트 미렌더 (Workbench 5단계가 default).

import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import type { IntentId, ModelEntry } from "../../ipc/catalog";

interface PromptTemplateStepProps {
  model: ModelEntry;
  intent: IntentId | null;
  /** "더 깊게 (LoRA로 조정)" 클릭 시 — Workbench 5단계 stepper로 전환 시그널. */
  onAdvanceToFineTune?: () => void;
}

interface SavedPattern {
  name: string;
  text: string;
  createdAt: string;
}

export function PromptTemplateStep({
  model,
  intent,
  onAdvanceToFineTune,
}: PromptTemplateStepProps) {
  const { t } = useTranslation();
  const [savedPatterns, setSavedPatterns] = useState<SavedPattern[]>([]);
  const [toast, setToast] = useState<string | null>(null);

  const storageKey = intent ? `lmmaster.prompts.${intent}` : null;

  // localStorage에서 저장된 패턴 로드.
  useEffect(() => {
    if (!storageKey) {
      setSavedPatterns([]);
      return;
    }
    try {
      const raw = localStorage.getItem(storageKey);
      const parsed = raw ? JSON.parse(raw) : [];
      setSavedPatterns(Array.isArray(parsed) ? parsed : []);
    } catch {
      setSavedPatterns([]);
    }
  }, [storageKey]);

  const flashToast = useCallback((msg: string) => {
    setToast(msg);
    window.setTimeout(() => setToast(null), 2200);
  }, []);

  const handleCopy = useCallback(
    async (text: string) => {
      try {
        await navigator.clipboard.writeText(text);
        flashToast(
          t(
            "screens.workbench.promptTemplate.copied",
            "프롬프트를 복사했어요. Chat 페이지에 붙여넣어 사용해 보세요.",
          ),
        );
      } catch (e) {
        console.warn("clipboard write failed:", e);
        flashToast(
          t(
            "screens.workbench.promptTemplate.copyFailed",
            "복사하지 못했어요.",
          ),
        );
      }
    },
    [flashToast, t],
  );

  const handleSave = useCallback(
    (text: string) => {
      if (!storageKey) return;
      const generatedName = `${t("screens.workbench.promptTemplate.savedAt", "저장")} ${new Date().toLocaleString("ko-KR")}`;
      const next: SavedPattern[] = [
        ...savedPatterns,
        { name: generatedName, text, createdAt: new Date().toISOString() },
      ];
      try {
        localStorage.setItem(storageKey, JSON.stringify(next));
        setSavedPatterns(next);
        flashToast(
          t(
            "screens.workbench.promptTemplate.saved",
            "내 패턴에 저장했어요.",
          ),
        );
      } catch (e) {
        console.warn("save failed:", e);
        flashToast(
          t(
            "screens.workbench.promptTemplate.saveFailed",
            "저장하지 못했어요.",
          ),
        );
      }
    },
    [savedPatterns, storageKey, flashToast, t],
  );

  const handleDelete = useCallback(
    (idx: number) => {
      if (!storageKey) return;
      const next = savedPatterns.filter((_, i) => i !== idx);
      try {
        localStorage.setItem(storageKey, JSON.stringify(next));
        setSavedPatterns(next);
      } catch (e) {
        console.warn("delete failed:", e);
      }
    },
    [savedPatterns, storageKey],
  );

  const examples = model.use_case_examples ?? [];

  return (
    <section
      className="workbench-prompt-template-step"
      role="region"
      aria-labelledby="workbench-prompt-template-heading"
      data-testid="workbench-prompt-template-step"
    >
      <header className="workbench-prompt-template-header">
        <h3
          id="workbench-prompt-template-heading"
          className="workbench-prompt-template-title"
        >
          {t(
            "screens.workbench.promptTemplate.title",
            "① 프롬프트 템플릿 — 즉시 시작",
          )}
        </h3>
        <p className="workbench-prompt-template-subtitle">
          {t(
            "screens.workbench.promptTemplate.subtitle",
            "이 모델로 바로 쓸 수 있는 작업 예시예요. 카드를 누르면 프롬프트가 복사돼요.",
          )}
        </p>
      </header>

      {examples.length === 0 ? (
        <p
          className="workbench-prompt-template-empty"
          data-testid="prompt-template-empty"
        >
          {t(
            "screens.workbench.promptTemplate.empty",
            "이 모델에는 아직 등록된 작업 예시가 없어요.",
          )}
        </p>
      ) : (
        <ul
          className="workbench-prompt-template-grid"
          data-testid="prompt-template-grid"
          aria-label={t(
            "screens.workbench.promptTemplate.gridLabel",
            "프롬프트 템플릿 목록",
          )}
        >
          {examples.map((text, idx) => (
            <li
              key={idx}
              className="workbench-prompt-template-card"
              data-testid={`prompt-template-card-${idx}`}
            >
              <p className="workbench-prompt-template-card-text">{text}</p>
              <div className="workbench-prompt-template-card-actions">
                <button
                  type="button"
                  className="workbench-prompt-template-action workbench-prompt-template-action-primary"
                  onClick={() => handleCopy(text)}
                  data-testid={`prompt-template-copy-${idx}`}
                >
                  {t("screens.workbench.promptTemplate.copy", "복사")}
                </button>
                {storageKey && (
                  <button
                    type="button"
                    className="workbench-prompt-template-action"
                    onClick={() => handleSave(text)}
                    data-testid={`prompt-template-save-${idx}`}
                  >
                    {t("screens.workbench.promptTemplate.save", "내 패턴 저장")}
                  </button>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}

      {storageKey && savedPatterns.length > 0 && (
        <section
          className="workbench-prompt-template-saved"
          aria-labelledby="workbench-prompt-template-saved-heading"
          data-testid="prompt-template-saved"
        >
          <h4
            id="workbench-prompt-template-saved-heading"
            className="workbench-prompt-template-saved-heading"
          >
            {t(
              "screens.workbench.promptTemplate.savedHeading",
              "내가 저장한 패턴",
            )}
          </h4>
          <ul className="workbench-prompt-template-saved-list">
            {savedPatterns.map((p, idx) => (
              <li
                key={idx}
                className="workbench-prompt-template-saved-item"
                data-testid={`prompt-template-saved-${idx}`}
              >
                <span className="workbench-prompt-template-saved-name">
                  {p.name}
                </span>
                <p className="workbench-prompt-template-saved-text">{p.text}</p>
                <div className="workbench-prompt-template-card-actions">
                  <button
                    type="button"
                    className="workbench-prompt-template-action workbench-prompt-template-action-primary"
                    onClick={() => handleCopy(p.text)}
                  >
                    {t("screens.workbench.promptTemplate.copy", "복사")}
                  </button>
                  <button
                    type="button"
                    className="workbench-prompt-template-action"
                    onClick={() => handleDelete(idx)}
                    data-testid={`prompt-template-delete-${idx}`}
                  >
                    {t("screens.workbench.promptTemplate.delete", "삭제")}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        </section>
      )}

      {onAdvanceToFineTune && (
        <div
          className="workbench-prompt-template-advance"
          data-testid="prompt-template-advance"
        >
          <p className="workbench-prompt-template-advance-hint">
            {t(
              "screens.workbench.promptTemplate.advanceHint",
              "프롬프트만으로 부족할 때 — LoRA 파인튜닝으로 더 정확하게 맞출 수 있어요.",
            )}
          </p>
          <button
            type="button"
            className="workbench-prompt-template-advance-cta"
            onClick={onAdvanceToFineTune}
            data-testid="prompt-template-advance-cta"
          >
            {t(
              "screens.workbench.promptTemplate.advance",
              "더 깊게 (LoRA로 조정) →",
            )}
          </button>
        </div>
      )}

      {toast && (
        <div
          className="workbench-prompt-template-toast"
          role="status"
          data-testid="prompt-template-toast"
        >
          {toast}
        </div>
      )}
    </section>
  );
}
