// Phase R-F.3 (ADR-0064 §F.3) — selected_path_token IPC helper.
//
// 정책:
// - 사용자가 dialog로 선택한 file/directory만 token으로 발급.
// - 발급된 token을 backend IPC (ingest_path, workbench_preview_jsonl 등)에 전달.
// - 24h soft TTL. 만료 시 한국어 카피로 재선택 안내.
// - localStorage 캐시 금지 — process restart 후 dangling pointer 위험.

import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

/** dialog 결과를 backend registry에 등록하고 token 반환. */
async function issuePathToken(
  path: string,
  kind: "file" | "directory",
): Promise<string> {
  return invoke<string>("issue_path_token", { path, kind });
}

/**
 * JSONL/JSON 파일 선택 dialog 열기 + token 발급.
 *
 * 반환:
 * - `{ token, name }`: 사용자 선택 + token 발급 성공.
 * - `null`: 사용자 dialog 취소 (graceful no-op).
 *
 * @throws dialog plugin 또는 backend canonicalize 실패.
 */
export async function pickJsonlFile(): Promise<{
  token: string;
  name: string;
} | null> {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "JSONL", extensions: ["jsonl", "json"] }],
  });
  if (!selected || typeof selected !== "string") return null;
  const token = await issuePathToken(selected, "file");
  const name = selected.split(/[\\/]/).pop() ?? selected;
  return { token, name };
}

/**
 * 디렉터리 선택 dialog 열기 + token 발급.
 *
 * 반환:
 * - `{ token, name }`: 사용자 선택 + token 발급 성공.
 * - `null`: 취소 (graceful).
 */
export async function pickDirectory(): Promise<{
  token: string;
  name: string;
} | null> {
  const selected = await open({
    multiple: false,
    directory: true,
  });
  if (!selected || typeof selected !== "string") return null;
  const token = await issuePathToken(selected, "directory");
  const name = selected.split(/[\\/]/).pop() ?? selected;
  return { token, name };
}
