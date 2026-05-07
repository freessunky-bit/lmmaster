// Phase R-J.2 (ADR-0064 §J) — i18n ko/en parity check.
//
// 정책:
// - ko.json / en.json의 flattened key path가 양쪽에 모두 존재해야 해요.
// - "en만 있는 키"가 있으면 한국어 화면에 영어가 fallback로 노출돼 사용자 혼란 발생.
// - "ko만 있는 키"가 있으면 영어 화면에 키 path가 그대로 노출.
//
// 사용:
//   node .claude/scripts/check-i18n-parity.mjs
//
// CI:
//   .github/workflows/ci.yml의 Node CI step에서 호출.
//
// Exit codes:
//   0 — parity OK
//   1 — parity 깨짐 (ko-only 또는 en-only 키 발견)
//   2 — 파일 read/parse 실패

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repo = path.resolve(__dirname, "..", "..");
const ko_path = path.join(repo, "apps", "desktop", "src", "i18n", "ko.json");
const en_path = path.join(repo, "apps", "desktop", "src", "i18n", "en.json");

function readJson(p) {
  if (!fs.existsSync(p)) {
    console.error(`i18n 파일이 없어요: ${p}`);
    process.exit(2);
  }
  try {
    return JSON.parse(fs.readFileSync(p, "utf8"));
  } catch (e) {
    console.error(`i18n JSON 파싱 실패 (${p}): ${e.message}`);
    process.exit(2);
  }
}

/**
 * 객체를 점-구분 flat key 집합으로 변환.
 * { a: { b: "x", c: 1 } } → ["a.b", "a.c"]
 */
function flatten(obj, prefix = "", out = new Set()) {
  if (obj === null || typeof obj !== "object") {
    out.add(prefix);
    return out;
  }
  if (Array.isArray(obj)) {
    // 배열은 leaf로 처리 (인덱스 비교 의미 없음).
    out.add(prefix);
    return out;
  }
  for (const [k, v] of Object.entries(obj)) {
    const next = prefix ? `${prefix}.${k}` : k;
    flatten(v, next, out);
  }
  return out;
}

const ko = readJson(ko_path);
const en = readJson(en_path);

const ko_keys = flatten(ko);
const en_keys = flatten(en);

const ko_only = [...ko_keys].filter((k) => !en_keys.has(k)).sort();
const en_only = [...en_keys].filter((k) => !ko_keys.has(k)).sort();

console.log(`ko.json: ${ko_keys.size} keys`);
console.log(`en.json: ${en_keys.size} keys`);

let exit_code = 0;

if (ko_only.length > 0) {
  console.error(`\n❌ ko에만 있는 키 (${ko_only.length}건):`);
  for (const k of ko_only) console.error(`  - ${k}`);
  exit_code = 1;
}

if (en_only.length > 0) {
  console.error(`\n❌ en에만 있는 키 (${en_only.length}건):`);
  for (const k of en_only) console.error(`  - ${k}`);
  exit_code = 1;
}

if (exit_code === 0) {
  console.log("\n✅ ko/en parity OK — 키 누락 0건");
} else {
  console.error(
    "\n각 i18n 파일은 ko + en 양쪽에 동시에 추가해야 해요 (CLAUDE.md §4.1).",
  );
}

process.exit(exit_code);
