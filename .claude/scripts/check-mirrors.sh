#!/usr/bin/env bash
# Probe known non-gated mirror candidates for each FAIL manifest entry.
# We check candidate repo's main page (HTML 200 = repo exists) AND a sample GGUF file.
# Output: id <TAB> candidate_repo <TAB> repo_status <TAB> sample_file <TAB> sample_status

set -u

UA="Mozilla/5.0 (LMmaster URL Check)"
PARALLEL=10

probe() {
  local id="$1"
  local repo="$2"
  local sample="$3"
  local repo_url="https://huggingface.co/${repo}"
  local file_url="https://huggingface.co/${repo}/resolve/main/${sample}"
  local rcode=$(curl -sLI -o /dev/null -w "%{http_code}" -m 15 -A "$UA" "$repo_url" 2>/dev/null || echo "000")
  local fcode=$(curl -sLI -o /dev/null -w "%{http_code}" -m 15 -A "$UA" "$file_url" 2>/dev/null || echo "000")
  printf "%s\t%s\t%s\t%s\t%s\n" "$id" "$repo" "$rcode" "$sample" "$fcode"
}

# Run probes in parallel
run() {
  while IFS=$'\t' read -r id repo sample; do
    probe "$id" "$repo" "$sample" &
    if (( $(jobs -r | wc -l) >= PARALLEL )); then wait -n; fi
  done
  wait
}

# Format: id <TAB> repo <TAB> sample
run <<'EOF'
aya-expanse-32b	bartowski/aya-expanse-32b-GGUF	aya-expanse-32b-Q4_K_M.gguf
aya-expanse-32b	mradermacher/aya-expanse-32b-GGUF	aya-expanse-32b.Q4_K_M.gguf
aya-expanse-8b	bartowski/aya-expanse-8b-GGUF	aya-expanse-8b-Q4_K_M.gguf
aya-expanse-8b	mradermacher/aya-expanse-8b-GGUF	aya-expanse-8b.Q4_K_M.gguf
deepseek-r1-7b	bartowski/DeepSeek-R1-Distill-Qwen-7B-GGUF	DeepSeek-R1-Distill-Qwen-7B-Q4_K_M.gguf
deepseek-r1-7b	unsloth/DeepSeek-R1-Distill-Qwen-7B-GGUF	DeepSeek-R1-Distill-Qwen-7B-Q4_K_M.gguf
exaone-4.0-1.2b-instruct	bartowski/LGAI-EXAONE_EXAONE-4.0-1.2B-GGUF	LGAI-EXAONE_EXAONE-4.0-1.2B-Q4_K_M.gguf
exaone-4.0-1.2b-instruct	unsloth/EXAONE-4.0-1.2B-GGUF	EXAONE-4.0-1.2B-Q4_K_M.gguf
exaone-4.0-32b-instruct	bartowski/LGAI-EXAONE_EXAONE-4.0-32B-GGUF	LGAI-EXAONE_EXAONE-4.0-32B-Q4_K_M.gguf
exaone-4.0-32b-instruct	unsloth/EXAONE-4.0-32B-GGUF	EXAONE-4.0-32B-Q4_K_M.gguf
hcx-seed-8b	bartowski/naver-hyperclovax_HyperCLOVAX-SEED-Text-Instruct-8B-GGUF	naver-hyperclovax_HyperCLOVAX-SEED-Text-Instruct-8B-Q4_K_M.gguf
hcx-seed-8b	mradermacher/HyperCLOVAX-SEED-Text-Instruct-8B-GGUF	HyperCLOVAX-SEED-Text-Instruct-8B.Q4_K_M.gguf
hcx-seed-1.5b	bartowski/naver-hyperclovax_HyperCLOVAX-SEED-Text-Instruct-1.5B-GGUF	naver-hyperclovax_HyperCLOVAX-SEED-Text-Instruct-1.5B-Q4_K_M.gguf
hcx-seed-1.5b	mradermacher/HyperCLOVAX-SEED-Text-Instruct-1.5B-GGUF	HyperCLOVAX-SEED-Text-Instruct-1.5B.Q4_K_M.gguf
kullm3	mradermacher/KULLM3-GGUF	KULLM3.Q4_K_M.gguf
kullm3	QuantFactory/KULLM3-GGUF	KULLM3.Q4_K_M.gguf
mistral-small-24b	bartowski/Mistral-Small-24B-Instruct-2501-GGUF	Mistral-Small-24B-Instruct-2501-Q4_K_M.gguf
mistral-small-24b	unsloth/Mistral-Small-24B-Instruct-2501-GGUF	Mistral-Small-24B-Instruct-2501-Q4_K_M.gguf
nemotron-3-nano-4b	unsloth/Nemotron-3-Nano-4B-GGUF	Nemotron-3-Nano-4B-Q4_K_M.gguf
nemotron-3-nano-4b	bartowski/nvidia_NVIDIA-Nemotron-3-Nano-4B-GGUF	nvidia_NVIDIA-Nemotron-3-Nano-4B-Q4_K_M.gguf
phi-4-14b	bartowski/phi-4-GGUF	phi-4-Q4_K_M.gguf
phi-4-14b	unsloth/phi-4-GGUF	phi-4-Q4_K_M.gguf
solar-10.7b-instruct	TheBloke/SOLAR-10.7B-Instruct-v1.0-GGUF	solar-10.7b-instruct-v1.0.Q4_K_M.gguf
solar-10.7b-instruct	bartowski/SOLAR-10.7B-Instruct-v1.0-GGUF	SOLAR-10.7B-Instruct-v1.0-Q4_K_M.gguf
yi-1.5-34b-chat	bartowski/Yi-1.5-34B-Chat-GGUF	Yi-1.5-34B-Chat-Q4_K_M.gguf
yi-1.5-34b-chat	mradermacher/Yi-1.5-34B-Chat-GGUF	Yi-1.5-34B-Chat.Q4_K_M.gguf
yi-1.5-9b-chat	bartowski/Yi-1.5-9B-Chat-GGUF	Yi-1.5-9B-Chat-Q4_K_M.gguf
yi-1.5-9b-chat	mradermacher/Yi-1.5-9B-Chat-GGUF	Yi-1.5-9B-Chat.Q4_K_M.gguf
yi-1.5-6b-chat	bartowski/Yi-1.5-6B-Chat-GGUF	Yi-1.5-6B-Chat-Q4_K_M.gguf
yi-1.5-6b-chat	mradermacher/Yi-1.5-6B-Chat-GGUF	Yi-1.5-6B-Chat.Q4_K_M.gguf
yi-ko-6b	mradermacher/Yi-Ko-6B-GGUF	Yi-Ko-6B.Q4_K_M.gguf
yi-ko-6b	QuantFactory/Yi-Ko-6B-GGUF	Yi-Ko-6B.Q4_K_M.gguf
deepseek-coder-v2-16b	bartowski/DeepSeek-Coder-V2-Lite-Instruct-GGUF	DeepSeek-Coder-V2-Lite-Instruct-Q4_K_M.gguf
deepseek-coder-v2-16b	mradermacher/DeepSeek-Coder-V2-Lite-Instruct-GGUF	DeepSeek-Coder-V2-Lite-Instruct.Q4_K_M.gguf
qwen-2.5-coder-7b-instruct	bartowski/Qwen2.5-Coder-7B-Instruct-GGUF	Qwen2.5-Coder-7B-Instruct-Q4_K_M.gguf
qwen-2.5-coder-7b-instruct	lmstudio-community/Qwen2.5-Coder-7B-Instruct-GGUF	Qwen2.5-Coder-7B-Instruct-Q4_K_M.gguf
bge-m3	CompendiumLabs/bge-m3-gguf	bge-m3-FP16.gguf
bge-m3	xtuner/bge-m3-gguf	bge-m3-fp16.gguf
kure-v1	mradermacher/KURE-v1-GGUF	KURE-v1.Q8_0.gguf
kure-v1	dragonkue/KURE-v1-GGUF	KURE-v1-FP16.gguf
nous-hermes-2-mistral-7b-dpo	NousResearch/Nous-Hermes-2-Mistral-7B-DPO	Nous-Hermes-2-Mistral-7B-DPO.Q4_K_M.gguf
nous-hermes-2-mistral-7b-dpo	TheBloke/Nous-Hermes-2-Mistral-7B-DPO-GGUF	nous-hermes-2-mistral-7b-dpo.Q4_K_M.gguf
polyglot-ko-12.8b	heegyu/polyglot-ko-12.8b-GGUF	polyglot-ko-12.8b-q4_K_M.gguf
polyglot-ko-12.8b	mradermacher/polyglot-ko-12.8b-GGUF	polyglot-ko-12.8b.Q4_K_M.gguf
stheno-l3-8b	bartowski/L3-8B-Stheno-v3.2-GGUF	L3-8B-Stheno-v3.2-Q4_K_M.gguf
stheno-l3-8b	mradermacher/L3-8B-Stheno-v3.2-GGUF	L3-8B-Stheno-v3.2.Q4_K_M.gguf
synatra-7b-v0.3-rp	bartowski/Synatra-7B-v0.3-RP-GGUF	Synatra-7B-v0.3-RP-Q4_K_M.gguf
synatra-7b-v0.3-rp	mradermacher/Synatra-7B-v0.3-RP-GGUF	Synatra-7B-v0.3-RP.Q4_K_M.gguf
EOF
