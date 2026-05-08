// Fetch HF tree JSON for a list of repos in parallel.
// 사용: node .claude/scripts/fetch-hf-trees.mjs <output-dir>

import fs from "node:fs";
import path from "node:path";

const outDir = process.argv[2] ?? "/tmp/hf-trees";
fs.mkdirSync(outDir, { recursive: true });

const repos = {
  // A 그룹
  "aya-expanse-32b": "bartowski/aya-expanse-32b-GGUF",
  "aya-expanse-8b": "bartowski/aya-expanse-8b-GGUF",
  "deepseek-r1-7b": "bartowski/DeepSeek-R1-Distill-Qwen-7B-GGUF",
  "mistral-small-24b": "bartowski/Mistral-Small-24B-Instruct-2501-GGUF",
  "phi-4-14b": "bartowski/phi-4-GGUF",
  "solar-10.7b": "TheBloke/SOLAR-10.7B-Instruct-v1.0-GGUF",
  "yi-1.5-34b": "bartowski/Yi-1.5-34B-Chat-GGUF",
  "yi-1.5-9b": "bartowski/Yi-1.5-9B-Chat-GGUF",
  "yi-1.5-6b": "bartowski/Yi-1.5-6B-Chat-GGUF",
  "qwen-2.5-coder-7b": "bartowski/Qwen2.5-Coder-7B-Instruct-GGUF",

  // B 그룹
  "exaone-4-1.2b": "LGAI-EXAONE/EXAONE-4.0-1.2B-GGUF",
  "exaone-4-32b": "LGAI-EXAONE/EXAONE-4.0-32B-GGUF",

  // C 그룹
  "bge-m3": "gpustack/bge-m3-GGUF",
  "kure-v1": "mykor/KURE-v1-gguf",
  "yi-ko-6b": "mradermacher/Yi-Ko-6B-GGUF",

  // D 그룹
  "deepseek-coder-v2-16b": "bartowski/DeepSeek-Coder-V2-Lite-Instruct-GGUF",
  "nous-hermes-2-mistral-7b-dpo": "mradermacher/Nous-Hermes-2-Mistral-7B-DPO-GGUF",
  "stheno-l3-8b": "bartowski/L3-8B-Stheno-v3.2-GGUF",
  "synatra-7b-v0.3-rp": "mradermacher/Synatra-7B-v0.3-RP-GGUF",

  // E 그룹
  "nemotron-3-nano-4b": "unsloth/NVIDIA-Nemotron-3-Nano-4B-GGUF",

  // F 그룹
  "kullm3": "QuantFactory/KULLM3-GGUF",
};

async function fetchTree(name, repo) {
  const url = `https://huggingface.co/api/models/${repo}/tree/main`;
  try {
    const res = await fetch(url);
    if (!res.ok) {
      console.error(`${name}: HTTP ${res.status} from ${url}`);
      return { name, ok: false, status: res.status };
    }
    const text = await res.text();
    const data = JSON.parse(text);
    fs.writeFileSync(path.join(outDir, `${name}.json`), text, "utf8");
    return { name, ok: true, count: data.length };
  } catch (e) {
    console.error(`${name}: ${e.message}`);
    return { name, ok: false, error: e.message };
  }
}

const results = await Promise.all(
  Object.entries(repos).map(([name, repo]) => fetchTree(name, repo)),
);

for (const r of results) {
  if (r.ok) {
    console.log(`OK ${r.name}: ${r.count} entries`);
  } else {
    console.log(`FAIL ${r.name}: ${r.status ?? r.error}`);
  }
}
