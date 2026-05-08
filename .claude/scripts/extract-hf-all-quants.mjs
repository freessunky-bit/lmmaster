// Extract per-manifest quant info — single Q4_K_M for most, multiple for EXAONE.
// Output: { id: { repo, quants: [ {label, file_path, sha256, size_mb} ] } }
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..");
const inDir = path.join(repoRoot, ".claude", "scripts", "_hf_trees");

const repos = {
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
  "exaone-4-1.2b": "LGAI-EXAONE/EXAONE-4.0-1.2B-GGUF",
  "exaone-4-32b": "LGAI-EXAONE/EXAONE-4.0-32B-GGUF",
  "bge-m3": "gpustack/bge-m3-GGUF",
  "kure-v1": "mykor/KURE-v1-gguf",
  "yi-ko-6b": "mradermacher/Yi-Ko-6B-GGUF",
  "deepseek-coder-v2-16b": "bartowski/DeepSeek-Coder-V2-Lite-Instruct-GGUF",
  "nous-hermes-2-mistral-7b-dpo": "mradermacher/Nous-Hermes-2-Mistral-7B-DPO-GGUF",
  "stheno-l3-8b": "bartowski/L3-8B-Stheno-v3.2-GGUF",
  "synatra-7b-v0.3-rp": "mradermacher/Synatra-7B-v0.3-RP-GGUF",
  "nemotron-3-nano-4b": "unsloth/NVIDIA-Nemotron-3-Nano-4B-GGUF",
  "kullm3": "QuantFactory/KULLM3-GGUF",
};

// Per-manifest desired quants. If empty, will default to Q4_K_M.
const desiredQuants = {
  "exaone-4-1.2b": ["Q4_K_M", "Q8_0"],
  "exaone-4-32b": ["Q4_K_M", "Q5_K_M"],
};

function findQuant(entries, quantLabel) {
  const ggufs = entries.filter((e) => e.type === "file" && /\.gguf$/i.test(e.path) && e.lfs);
  // Match patterns like *-Q4_K_M.gguf, *.Q4_K_M.gguf, *_Q4_K_M.gguf
  const re = new RegExp(`(^|[-._])${quantLabel}([-._]|\\.gguf$)`, "i");
  return ggufs.find((f) => re.test(f.path));
}

const results = {};
for (const [name, repo] of Object.entries(repos)) {
  const filePath = path.join(inDir, `${name}.json`);
  const tree = JSON.parse(fs.readFileSync(filePath, "utf8"));
  const wanted = desiredQuants[name] ?? ["Q4_K_M"];
  const quants = [];
  for (const q of wanted) {
    const f = findQuant(tree, q);
    if (f) {
      quants.push({
        label: q,
        file_path: f.path,
        sha256: f.lfs.oid,
        size_bytes: f.lfs.size,
        size_mb: Math.round(f.lfs.size / 1048576),
      });
    } else {
      console.error(`MISSING ${name}: quant ${q} not found in ${repo}`);
    }
  }
  results[name] = { repo, quants };
}

fs.writeFileSync(
  path.join(inDir, "_summary_full.json"),
  JSON.stringify(results, null, 2),
  "utf8",
);
console.log(JSON.stringify(results, null, 2));
