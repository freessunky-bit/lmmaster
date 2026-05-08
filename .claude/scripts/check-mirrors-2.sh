#!/usr/bin/env bash
# Round 2: probe extra candidates for tricky cases (still 401 or wrong filename).
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
hcx-seed-1.5b	QuantFactory/HyperCLOVAX-SEED-Text-Instruct-1.5B-GGUF	HyperCLOVAX-SEED-Text-Instruct-1.5B.Q4_K_M.gguf
hcx-seed-1.5b	tensorblock/HyperCLOVAX-SEED-Text-Instruct-1.5B-GGUF	HyperCLOVAX-SEED-Text-Instruct-1.5B-Q4_K_M.gguf
hcx-seed-1.5b	heegyu/HyperCLOVAX-SEED-Text-Instruct-1.5B-GGUF	HyperCLOVAX-SEED-Text-Instruct-1.5B.Q4_K_M.gguf
hcx-seed-8b	QuantFactory/HyperCLOVAX-SEED-Text-Instruct-8B-GGUF	HyperCLOVAX-SEED-Text-Instruct-8B.Q4_K_M.gguf
hcx-seed-8b	tensorblock/HyperCLOVAX-SEED-Text-Instruct-8B-GGUF	HyperCLOVAX-SEED-Text-Instruct-8B-Q4_K_M.gguf
hcx-seed-8b	heegyu/HyperCLOVAX-SEED-Text-Instruct-8B-GGUF	HyperCLOVAX-SEED-Text-Instruct-8B.Q4_K_M.gguf
kure-v1	heegyu/KURE-v1-GGUF	KURE-v1.Q4_K_M.gguf
kure-v1	QuantFactory/KURE-v1-GGUF	KURE-v1.Q4_K_M.gguf
kure-v1	dragonkue/bge-reranker-v2-m3-ko-GGUF	bge-reranker-v2-m3-ko.Q4_K_M.gguf
nemotron-3-nano-4b	tensorblock/NVIDIA-Nemotron-3-Nano-4B-GGUF	NVIDIA-Nemotron-3-Nano-4B-Q4_K_M.gguf
nemotron-3-nano-4b	QuantFactory/NVIDIA-Nemotron-3-Nano-4B-GGUF	NVIDIA-Nemotron-3-Nano-4B-Q4_K_M.gguf
nemotron-3-nano-4b	bartowski/Nemotron-3-Nano-4B-GGUF	Nemotron-3-Nano-4B-Q4_K_M.gguf
nemotron-3-nano-4b	unsloth/NVIDIA-Nemotron-3-Nano-4B-GGUF	NVIDIA-Nemotron-3-Nano-4B-Q4_K_M.gguf
polyglot-ko-12.8b	QuantFactory/polyglot-ko-12.8b-GGUF	polyglot-ko-12.8b.Q4_K_M.gguf
polyglot-ko-12.8b	tensorblock/polyglot-ko-12.8b-GGUF	polyglot-ko-12.8b-Q4_K_M.gguf
polyglot-ko-12.8b	heegyu/polyglot-ko-12.8b	polyglot-ko-12.8b.Q4_K_M.gguf
yi-ko-6b	mradermacher/Yi-Ko-6B-GGUF	Yi-Ko-6B.Q4_0.gguf
yi-ko-6b	tensorblock/Yi-Ko-6B-GGUF	Yi-Ko-6B-Q4_K_M.gguf
yi-ko-6b	heegyu/Yi-Ko-6B-GGUF	Yi-Ko-6B.Q4_K_M.gguf
yi-ko-6b	beomi/Yi-Ko-6B	Yi-Ko-6B-Q4_K_M.gguf
nous-hermes-2-mistral-7b-dpo	bartowski/Nous-Hermes-2-Mistral-7B-DPO-GGUF	Nous-Hermes-2-Mistral-7B-DPO-Q4_K_M.gguf
nous-hermes-2-mistral-7b-dpo	mradermacher/Nous-Hermes-2-Mistral-7B-DPO-GGUF	Nous-Hermes-2-Mistral-7B-DPO.Q4_K_M.gguf
exaone-4.0-1.2b-instruct	LGAI-EXAONE/EXAONE-4.0-1.2B-GGUF	EXAONE-4.0-1.2B-Q4_K_M.gguf
exaone-4.0-32b-instruct	LGAI-EXAONE/EXAONE-4.0-32B-GGUF	EXAONE-4.0-32B-Q4_K_M.gguf
bge-m3	CompendiumLabs/bge-m3-gguf	bge-m3-fp16.gguf
bge-m3	CompendiumLabs/bge-m3-gguf	bge-m3-Q8_0.gguf
bge-m3	CompendiumLabs/bge-m3-gguf	bge-m3-Q4_K_M.gguf
EOF
wait
