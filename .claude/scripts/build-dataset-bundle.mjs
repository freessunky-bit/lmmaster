// build-dataset-bundle.mjs — Phase 23'.c (ADR-0061 §5).
//
// 동작:
// 1. manifests/snapshot/datasets/**/*.json 재귀 walk.
// 2. 각 file의 entries 합본.
// 3. 중복 id 검사 (deterministic).
// 4. id 알파벳 정렬.
// 5. manifests/apps/datasets-bundle.json에 single bundle 출력.
//
// build-catalog-bundle.mjs (모델 카탈로그) 패턴 미러.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repo = path.resolve(__dirname, "..", "..");

const SOURCE_DIR = path.join(repo, "manifests/snapshot/datasets");
const OUT_PATH = path.join(repo, "manifests/apps/datasets-bundle.json");

function* walk(dir) {
  if (!fs.existsSync(dir)) return;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const ent of entries) {
    const full = path.join(dir, ent.name);
    if (ent.isDirectory()) {
      yield* walk(full);
    } else if (ent.isFile() && ent.name.endsWith(".json")) {
      yield full;
    }
  }
}

const allEntries = [];
const seenIds = new Set();
let fileCount = 0;
for (const file of walk(SOURCE_DIR)) {
  fileCount += 1;
  const text = fs.readFileSync(file, "utf8");
  let parsed;
  try {
    parsed = JSON.parse(text);
  } catch (err) {
    console.error(`JSON parse 실패: ${file}\n${err.message}`);
    process.exit(1);
  }
  if (parsed.schema_version !== 1) {
    console.error(`schema_version != 1: ${file}`);
    process.exit(1);
  }
  if (!Array.isArray(parsed.entries)) {
    console.error(`entries 배열이 아니에요: ${file}`);
    process.exit(1);
  }
  for (const entry of parsed.entries) {
    if (!entry.id || typeof entry.id !== "string") {
      console.error(`id 누락 또는 잘못된 형식: ${file}`);
      process.exit(1);
    }
    if (seenIds.has(entry.id)) {
      console.error(`중복 dataset id: ${entry.id} (file: ${file})`);
      process.exit(1);
    }
    seenIds.add(entry.id);
    allEntries.push(entry);
  }
}

allEntries.sort((a, b) => a.id.localeCompare(b.id));

const bundle = {
  schema_version: 1,
  generated_at: new Date().toISOString(),
  entries: allEntries,
};

const outDir = path.dirname(OUT_PATH);
if (!fs.existsSync(outDir)) {
  fs.mkdirSync(outDir, { recursive: true });
}
fs.writeFileSync(OUT_PATH, JSON.stringify(bundle, null, 2) + "\n", "utf8");

console.log(
  `OK: ${OUT_PATH} entries=${allEntries.length} (${fileCount} files walked)`,
);
