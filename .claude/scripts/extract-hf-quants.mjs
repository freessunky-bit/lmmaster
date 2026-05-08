// Extract preferred quant info (Q4_K_M priority) from fetched HF tree JSONs.
// Output: a single combined JSON with name -> { repo, file_path, sha256, size_bytes, size_mb }.
import fs from "node:fs";
import path from "node:path";

const inDir = process.argv[2] ?? "/tmp/hf-trees";

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

// Preferred quant suffixes order. Q4_K_M is highest, then Q4_K_S, Q4_0, Q5_K_M, Q4_K, Q5_0, Q8_0, F16.
const priority = ["Q4_K_M", "Q4_K_S", "Q4_0", "Q5_K_M", "Q4_K", "Q5_0", "Q8_0", "F16"];

function findBest(entries, modelName) {
  // Filter to .gguf files only
  const ggufFiles = entries.filter((e) => e.type === "file" && /\.gguf$/i.test(e.path) && e.lfs);

  // Special case: bge-m3 from gpustack — small embedding, may be Q8_0 only
  // Check by priority — first match wins.
  for (const pref of priority) {
    // Pattern can be "-Q4_K_M.", ".Q4_K_M.", "_Q4_K_M.", "Q4_K_M-..gguf" (separator = - . _ or beginning)
    const re = new RegExp(`(^|[-._])${pref}([-._]|\\.gguf$)`, "i");
    const match = ggufFiles.find((f) => re.test(f.path));
    if (match) return { quant: pref, file: match };
  }

  // Fallback: any .gguf file (return first sorted by size, smaller usually preferred)
  if (ggufFiles.length > 0) {
    ggufFiles.sort((a, b) => a.size - b.size);
    return { quant: "first-available", file: ggufFiles[0] };
  }

  return null;
}

const results = {};
for (const [name, repo] of Object.entries(repos)) {
  const file = path.join(inDir, `${name}.json`);
  if (!fs.existsSync(file)) {
    console.error(`${name}: missing ${file}`);
    continue;
  }
  const tree = JSON.parse(fs.readFileSync(file, "utf8"));
  const found = findBest(tree, name);
  if (!found) {
    console.error(`${name}: NO gguf found in repo ${repo}`);
    results[name] = { repo, error: "no-gguf" };
    continue;
  }
  const sizeBytes = found.file.lfs.size;
  const sizeMb = Math.round(sizeBytes / 1048576);
  results[name] = {
    repo,
    quant: found.quant,
    file_path: found.file.path,
    sha256: found.file.lfs.oid,
    size_bytes: sizeBytes,
    size_mb: sizeMb,
  };
}

console.log(JSON.stringify(results, null, 2));
fs.writeFileSync(path.join(inDir, "_summary.json"), JSON.stringify(results, null, 2), "utf8");
