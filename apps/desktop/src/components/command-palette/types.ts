// Command Palette 도메인 타입. Phase 1A.4.e §B5.

export type CommandGroup = "wizard" | "navigation" | "system" | "diagnostics";

export interface Command {
  /** 전역 고유 id — 등록 해제 시 key. */
  id: string;
  /** 그룹 라벨링용 — palette UI에서 헤딩으로 묶음. */
  group: CommandGroup;
  /** 표시 라벨 (한국어 해요체). */
  label: string;
  /** fuzzy 검색 keywords — EN alias + jamo cheat. label과 union으로 substring 매칭. */
  keywords?: string[];
  /** 단축키 hint — 예: ["⌘", "K"]. tabular-nums로 우측 정렬. */
  shortcut?: string[];
  /** 실행 함수. async 가능 — palette는 close on resolve. */
  perform: () => void | Promise<void>;
  /** false 반환 시 회색 + selectable 안 됨. 검색 결과엔 노출. */
  isAvailable?: () => boolean;
}
