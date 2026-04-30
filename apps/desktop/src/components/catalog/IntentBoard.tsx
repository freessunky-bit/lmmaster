// IntentBoard — 의도 기반 모델 추천 1차 입력. (Phase 11'.b, ADR-0048)
//
// 정책:
// - SSOT는 `crates/shared-types/src/intents.rs::INTENT_VOCABULARY`. TS 미러는 ipc/catalog.ts.
// - 칩 11종 + "전체" 토글 = 12 radio. role=radiogroup. focus-visible ring 토큰.
// - 한국어 라벨은 INTENT_VOCABULARY default. i18n 키가 있으면 override (영어 locale 등).
// - prefers-reduced-motion: CSS 토큰 차원에서 자동 비활성.

import { useTranslation } from "react-i18next";

import { type IntentId } from "../../ipc/catalog";

/**
 * Frontend Intent 사전 — SSOT는 `crates/shared-types/src/intents.rs`.
 * 본 배열은 화면 표시 순서 + 한국어 라벨만 보유. id 11종은 IntentId enum과 일치해야 함
 * (TS 컴파일러가 enum 타입으로 1:1 검증).
 */
export const INTENT_VOCABULARY: ReadonlyArray<readonly [IntentId, string]> = [
  ["vision-image", "이미지 분석"],
  ["vision-multimodal", "이미지+텍스트 멀티모달"],
  ["translation-ko-en", "한↔영 번역"],
  ["translation-multi", "다국어 번역"],
  ["coding-general", "코딩"],
  ["coding-fim", "코드 자동완성 (FIM)"],
  ["agent-tool-use", "에이전트 / 도구 사용"],
  ["roleplay-narrative", "롤플레이 / 서사"],
  ["ko-conversation", "한국어 대화"],
  ["ko-rag", "한국어 RAG"],
  ["voice-stt", "음성 인식"],
];

interface IntentBoardProps {
  selected: IntentId | null;
  onSelect: (intent: IntentId | null) => void;
}

export function IntentBoard({ selected, onSelect }: IntentBoardProps) {
  const { t } = useTranslation();
  const headingLabel = t("catalog.intent.heading", "어떤 AI를 찾고 있어요?");
  return (
    <section
      className="intent-board"
      aria-labelledby="intent-board-heading"
      data-testid="intent-board"
    >
      <h3 id="intent-board-heading" className="intent-board-heading">
        {headingLabel}
      </h3>
      <div
        role="radiogroup"
        aria-label={headingLabel}
        className="intent-board-chips"
      >
        <button
          type="button"
          role="radio"
          aria-checked={selected === null}
          className={`intent-chip${selected === null ? " is-active" : ""}`}
          data-testid="intent-chip-all"
          onClick={() => onSelect(null)}
        >
          {t("catalog.intent.all", "전체")}
        </button>
        {INTENT_VOCABULARY.map(([id, koLabel]) => (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={selected === id}
            className={`intent-chip${selected === id ? " is-active" : ""}`}
            data-testid={`intent-chip-${id}`}
            onClick={() => onSelect(id)}
          >
            {t(`catalog.intent.${id}`, koLabel)}
          </button>
        ))}
      </div>
    </section>
  );
}
