#!/usr/bin/env bash
set -u
UA="Mozilla/5.0 (LMmaster URL Check)"

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

while IFS=$'\t' read -r id repo sample; do
  probe "$id" "$repo" "$sample" &
  if (( $(jobs -r | wc -l) >= 10 )); then wait -n; fi
done <<'EOF'
bge-m3	CompendiumLabs/bge-m3-gguf	bge-m3-FP16.gguf
bge-m3	CompendiumLabs/bge-m3-gguf	bge-m3.gguf
bge-m3	gpustack/bge-m3-GGUF	bge-m3-Q4_K_M.gguf
bge-m3	gpustack/bge-m3-GGUF	bge-m3-FP16.gguf
bge-m3	lm-kit/bge-m3-gguf	bge-m3-F16.gguf
bge-m3	lm-kit/bge-m3-gguf	bge-m3-Q8_0.gguf
hcx-seed-1.5b	naver-hyperclovax/HyperCLOVAX-SEED-Text-Instruct-1.5B	model.safetensors
hcx-seed-1.5b	naver-hyperclovax/HyperCLOVAX-SEED-Text-Instruct-1.5B-GGUF	HyperCLOVAX-SEED-Text-Instruct-1.5B.Q4_K_M.gguf
hcx-seed-8b	naver-hyperclovax/HyperCLOVAX-SEED-Text-Instruct-1.5B-GGUF	HyperCLOVAX-SEED-Text-Instruct-1.5B-Q4_K_M.gguf
nemotron-3-nano-4b	unsloth/NVIDIA-Nemotron-3-Nano-4B-GGUF	NVIDIA-Nemotron-3-Nano-4B-Q4_K_M.gguf
nemotron-3-nano-4b	unsloth/NVIDIA-Nemotron-3-Nano-4B-GGUF	NVIDIA-Nemotron-3-Nano-4B-Q5_K_M.gguf
nemotron-3-nano-4b	unsloth/NVIDIA-Nemotron-3-Nano-4B-GGUF	NVIDIA-Nemotron-3-Nano-4B-Q8_0.gguf
yi-ko-6b	tensorblock/Yi-Ko-6B-GGUF	Yi-Ko-6B-Q4_0.gguf
yi-ko-6b	tensorblock/Yi-Ko-6B-GGUF	Yi-Ko-6B-Q5_K_M.gguf
yi-ko-6b	mradermacher/Yi-Ko-6B-GGUF	Yi-Ko-6B.Q5_K_M.gguf
yi-ko-6b	mradermacher/Yi-Ko-6B-GGUF	Yi-Ko-6B.Q8_0.gguf
EOF
wait
