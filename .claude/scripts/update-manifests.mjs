// Update each target manifest with new repo + file_path + sha256 + size_mb.
// Preserves all other fields (description, intents, domain_scores, mmproj, tier, ...).
// Operates only on entries[0] (single entry per manifest).
//
// 사용:
//   node .claude/scripts/update-manifests.mjs
//
// Quant 정책:
// - 대부분의 manifest는 Q4_K_M 단일 quant.
// - EXAONE-4 1.2B / 32B는 기존 quant 2개를 보존(Q4_K_M + Q8_0 또는 Q5_K_M).
// - source.{repo,file}, quantization_options[].{file_path,sha256,size_mb}, install_size_mb 갱신.
// - 그 외 필드 (description, intents, etc.) 보존.

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..");
const summaryPath = path.join(repoRoot, ".claude", "scripts", "_hf_trees", "_summary_full.json");

const summary = JSON.parse(fs.readFileSync(summaryPath, "utf8"));

const manifestPaths = {
  "aya-expanse-32b": "manifests/snapshot/models/agents/aya-expanse-32b.json",
  "aya-expanse-8b": "manifests/snapshot/models/agents/aya-expanse-8b.json",
  "deepseek-r1-7b": "manifests/snapshot/models/agents/deepseek-r1-7b.json",
  "mistral-small-24b": "manifests/snapshot/models/agents/mistral-small-24b.json",
  "phi-4-14b": "manifests/snapshot/models/agents/phi-4-14b.json",
  "solar-10.7b": "manifests/snapshot/models/agents/solar-10.7b.json",
  "yi-1.5-34b": "manifests/snapshot/models/agents/yi-1.5-34b.json",
  "yi-1.5-9b": "manifests/snapshot/models/slm/yi-1.5-9b.json",
  "yi-1.5-6b": "manifests/snapshot/models/slm/yi-1.5-6b.json",
  "qwen-2.5-coder-7b": "manifests/snapshot/models/coding/qwen-2.5-coder-7b.json",
  "exaone-4-1.2b": "manifests/snapshot/models/agents/exaone-4-1.2b.json",
  "exaone-4-32b": "manifests/snapshot/models/coding/exaone-4-32b.json",
  "bge-m3": "manifests/snapshot/models/embeddings/bge-m3.json",
  "kure-v1": "manifests/snapshot/models/embeddings/kure-v1.json",
  "yi-ko-6b": "manifests/snapshot/models/agents/yi-ko-6b.json",
  "deepseek-coder-v2-16b": "manifests/snapshot/models/coding/deepseek-coder-v2-16b.json",
  "nous-hermes-2-mistral-7b-dpo":
    "manifests/snapshot/models/roleplay/nous-hermes-2-mistral-7b-dpo.json",
  "stheno-l3-8b": "manifests/snapshot/models/roleplay/stheno-l3-8b.json",
  "synatra-7b-v0.3-rp": "manifests/snapshot/models/roleplay/synatra-7b-v0.3-rp.json",
  "nemotron-3-nano-4b": "manifests/snapshot/models/agents/nemotron-3-nano-4b.json",
  "kullm3": "manifests/snapshot/models/agents/kullm3.json",
};

let okCount = 0;
let failCount = 0;
const log = [];

for (const [name, info] of Object.entries(summary)) {
  if (!info.quants || info.quants.length === 0) {
    console.error(`SKIP ${name}: no quants extracted`);
    failCount++;
    continue;
  }
  const relPath = manifestPaths[name];
  if (!relPath) {
    console.error(`SKIP ${name}: no manifest path mapping`);
    failCount++;
    continue;
  }
  const absPath = path.join(repoRoot, relPath);
  if (!fs.existsSync(absPath)) {
    console.error(`SKIP ${name}: manifest file missing ${absPath}`);
    failCount++;
    continue;
  }

  const body = JSON.parse(fs.readFileSync(absPath, "utf8"));
  if (!Array.isArray(body.entries) || body.entries.length === 0) {
    console.error(`SKIP ${name}: no entries array`);
    failCount++;
    continue;
  }
  const entry = body.entries[0];
  if (!entry.source || !Array.isArray(entry.quantization_options)) {
    console.error(`SKIP ${name}: missing source or quantization_options`);
    failCount++;
    continue;
  }

  const primary = info.quants[0];

  // Update source — point to primary quant.
  entry.source = {
    type: entry.source.type ?? "hugging-face",
    repo: info.repo,
    file: primary.file_path,
  };

  // Replace quantization_options entirely with new entries based on summary.
  // Existing labels in manifest are matched against summary labels — preserve label name as-is.
  const existingByLabel = new Map();
  for (const q of entry.quantization_options) {
    existingByLabel.set(q.label, q);
  }

  const newQuants = info.quants.map((q) => {
    const existing = existingByLabel.get(q.label);
    if (existing) {
      return {
        ...existing,
        label: q.label,
        size_mb: q.size_mb,
        sha256: q.sha256,
        file_path: q.file_path,
      };
    }
    // No existing quant of this label — build minimal entry.
    return {
      label: q.label,
      size_mb: q.size_mb,
      sha256: q.sha256,
      file_path: q.file_path,
    };
  });

  entry.quantization_options = newQuants;

  // install_size_mb sync with primary quant size.
  entry.install_size_mb = primary.size_mb;

  // Bump generated_at.
  body.generated_at = "2026-05-08T00:00:00Z";

  fs.writeFileSync(absPath, JSON.stringify(body, null, 2) + "\n", "utf8");
  const quantSummary = info.quants
    .map((q) => `${q.label}=${q.size_mb}MB`)
    .join(", ");
  console.log(`OK ${name}: ${info.repo} (${quantSummary})`);
  log.push({ name, repo: info.repo, quants: info.quants.map((q) => q.label) });
  okCount++;
}

console.log(`\n총 ${okCount}개 갱신, ${failCount}개 실패`);
fs.writeFileSync(
  path.join(repoRoot, ".claude", "scripts", "_hf_trees", "_update_log.json"),
  JSON.stringify(log, null, 2),
  "utf8",
);
