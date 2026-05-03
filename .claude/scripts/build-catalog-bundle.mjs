// Build catalog bundle — manifests/snapshot/models/**/*.json 들의 모든 entries를 단일
// manifests/apps/catalog.json 으로 합쳐 빌드.
//
// 정책 (Phase 13'.a — live model catalog refresh):
// - registry-fetcher가 "catalog" manifest_id로 단일 파일을 받음.
// - dev 모드는 여전히 per-file 디렉터리에서 직접 로드 (lib.rs::load_bundled_catalog).
// - manifest 추가/수정 후엔 본 스크립트 재실행.
//
// 사용:
//   node .claude/scripts/build-catalog-bundle.mjs

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repo = path.resolve(__dirname, "..", "..");
const src = path.join(repo, "manifests", "snapshot", "models");
const dst = path.join(repo, "manifests", "apps", "catalog.json");

if (!fs.existsSync(src)) {
  console.error(`source 디렉터리가 없어요: ${src}`);
  process.exit(1);
}

// Phase 11'.a (ADR-0048) — Intent 사전을 shared-types/src/intents.rs에서 직접 파싱.
// SSOT를 Rust 쪽에 유지하고, JS는 build 시점 검증만 수행 (drift 자동 방지).
function parseIntentVocab() {
  const p = path.join(repo, "crates", "shared-types", "src", "intents.rs");
  if (!fs.existsSync(p)) {
    console.error(`Intent 사전이 없어요: ${p}`);
    process.exit(1);
  }
  const text = fs.readFileSync(p, "utf8");
  const startMarker = "INTENT_VOCABULARY:";
  const startIdx = text.indexOf(startMarker);
  if (startIdx < 0) {
    console.error("INTENT_VOCABULARY 식별자를 찾지 못했어요.");
    process.exit(1);
  }
  const closeIdx = text.indexOf("];", startIdx);
  if (closeIdx < 0) {
    console.error("INTENT_VOCABULARY 끝 `];` 를 찾지 못했어요.");
    process.exit(1);
  }
  const slice = text.slice(startIdx, closeIdx);
  // 매치: ("vision-image", "이미지 분석"),
  const re = /\("([a-z][a-z0-9-]*)"\s*,\s*"([^"]+)"\)/g;
  const out = new Set();
  let m;
  while ((m = re.exec(slice)) !== null) {
    out.add(m[1]);
  }
  if (out.size === 0) {
    console.error("Intent 사전이 비어있어요. INTENT_VOCABULARY 정규식 매치 실패.");
    process.exit(1);
  }
  return out;
}

const intentVocab = parseIntentVocab();
console.log(`Intent 사전 ${intentVocab.size}종 로드`);

function validateEntry(e) {
  // serde(default) 호환 — 누락은 정상.
  const intents = Array.isArray(e.intents) ? e.intents : [];
  const scores =
    e.domain_scores && typeof e.domain_scores === "object" ? e.domain_scores : {};

  // intents 모두 사전 등록.
  const seen = new Set();
  for (const iid of intents) {
    if (!intentVocab.has(iid)) {
      console.error(`${e.id}: intent '${iid}'은(는) 사전에 등록되지 않았어요.`);
      process.exit(1);
    }
    if (seen.has(iid)) {
      console.error(`${e.id}: intent '${iid}'이(가) 중복돼 있어요.`);
      process.exit(1);
    }
    seen.add(iid);
  }

  // domain_scores 검증.
  for (const [iid, score] of Object.entries(scores)) {
    if (!intentVocab.has(iid)) {
      console.error(
        `${e.id}: domain_scores 키 '${iid}'은(는) 사전에 등록되지 않았어요.`,
      );
      process.exit(1);
    }
    if (typeof score !== "number" || score < 0 || score > 100) {
      console.error(
        `${e.id}: domain_scores '${iid}'=${score}는 0..=100 범위를 벗어나요.`,
      );
      process.exit(1);
    }
  }

  // Phase 13'.h.2.c (ADR-0051) — mmproj 필드 검증.
  if (e.mmproj !== undefined && e.mmproj !== null) {
    const m = e.mmproj;
    // url: https + huggingface.co 또는 github.com 화이트리스트.
    if (typeof m.url !== "string" || !/^https:\/\//.test(m.url)) {
      console.error(`${e.id}: mmproj.url은 https URL이어야 해요. 받은 값: ${m.url}`);
      process.exit(1);
    }
    if (
      !/^https:\/\/(huggingface\.co|github\.com)\//.test(m.url)
    ) {
      console.error(
        `${e.id}: mmproj.url은 huggingface.co 또는 github.com 도메인만 허용 — ${m.url}`,
      );
      process.exit(1);
    }
    // size_mb 양수 정수.
    if (
      typeof m.size_mb !== "number" ||
      !Number.isInteger(m.size_mb) ||
      m.size_mb <= 0
    ) {
      console.error(`${e.id}: mmproj.size_mb는 양의 정수여야 해요. 받은 값: ${m.size_mb}`);
      process.exit(1);
    }
    // sha256: null 또는 64-char hex.
    if (m.sha256 !== undefined && m.sha256 !== null) {
      if (typeof m.sha256 !== "string" || !/^[0-9a-f]{64}$/i.test(m.sha256)) {
        console.error(
          `${e.id}: mmproj.sha256은 64자 hex 또는 null이어야 해요. 받은 값: ${m.sha256}`,
        );
        process.exit(1);
      }
    }
    // precision: f16 / bf16 / f32 또는 미지정.
    if (m.precision !== undefined && m.precision !== null) {
      if (!["f16", "bf16", "f32"].includes(m.precision)) {
        console.error(
          `${e.id}: mmproj.precision은 f16/bf16/f32 중 하나여야 해요. 받은 값: ${m.precision}`,
        );
        process.exit(1);
      }
    }
    // source: 큐레이터 출처 (known set + null OK).
    const knownSources = ["bartowski", "ggml-org", "unsloth", "lmstudio-community", "Mungert"];
    if (m.source !== undefined && m.source !== null) {
      if (!knownSources.includes(m.source)) {
        console.warn(
          `${e.id}: mmproj.source '${m.source}'은(는) 알려진 큐레이터가 아니에요 (warning only).`,
        );
      }
    }
  }

  // vision_support=true + tier=verified면 mmproj 권장 (warning, v1.x에 강제 검토).
  if (e.vision_support === true && (e.tier === "verified" || e.tier === undefined)) {
    if (e.mmproj === undefined || e.mmproj === null) {
      console.warn(
        `${e.id}: vision_support=true이지만 mmproj 필드가 없어요 (llama.cpp 사용자에 한국어 안내 노출).`,
      );
    }
  }
}

function walkJson(dir) {
  const out = [];
  for (const name of fs.readdirSync(dir)) {
    const p = path.join(dir, name);
    const stat = fs.statSync(p);
    if (stat.isDirectory()) {
      out.push(...walkJson(p));
    } else if (name.endsWith(".json")) {
      out.push(p);
    }
  }
  return out;
}

const files = walkJson(src).sort();
const entries = [];
for (const f of files) {
  const body = JSON.parse(fs.readFileSync(f, "utf8"));
  if (body.schema_version !== 1) {
    console.warn(`schema_version != 1, skip: ${f}`);
    continue;
  }
  if (!Array.isArray(body.entries)) {
    console.warn(`entries 필드 없음, skip: ${f}`);
    continue;
  }
  for (const e of body.entries) {
    validateEntry(e);
    entries.push(e);
  }
}

console.log(`수집된 entries: ${entries.length} 개`);

// 중복 id 검사 — fail.
const ids = entries.map((e) => e.id);
const seen = new Map();
for (const id of ids) {
  seen.set(id, (seen.get(id) ?? 0) + 1);
}
const dup = [...seen.entries()].filter(([, c]) => c > 1);
if (dup.length > 0) {
  console.error(`중복 id 발견: ${dup.map(([id]) => id).join(", ")}`);
  process.exit(1);
}

// id 알파벳 순 — deterministic build.
entries.sort((a, b) => a.id.localeCompare(b.id));

const bundle = {
  $schema_hint:
    "model-registry ModelManifest schema_version=1. Auto-generated by build-catalog-bundle.mjs — do not edit by hand.",
  schema_version: 1,
  generated_at: new Date().toISOString().replace(/\.\d+Z$/, "Z"),
  entries,
};

const json = JSON.stringify(bundle, null, 2);
fs.writeFileSync(dst, json, "utf8");
console.log(`OK: ${dst} entries=${entries.length}`);
