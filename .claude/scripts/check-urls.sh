#!/usr/bin/env bash
# HEAD-check every URL in url-check-list.tsv with parallel curl.
# TSV input cols: id <TAB> kind <TAB> url <TAB> manifest
# Output cols:    id <TAB> kind <TAB> http_code <TAB> url <TAB> manifest

set -u

INPUT="${1:-.claude/scripts/url-check-list.tsv}"
OUT="${2:-.claude/scripts/url-check-results.tsv}"
PARALLEL="${3:-10}"

> "$OUT"

# Spawn background curls; gather PIDs.
i=0
pids=()
results_dir=$(mktemp -d)

while IFS=$'\t' read -r id kind url manifest; do
  (
    code=$(curl -sLI -o /dev/null -w "%{http_code}" -m 30 -A "Mozilla/5.0 (LMmaster URL Check)" "$url" 2>/dev/null || echo "000")
    printf "%s\t%s\t%s\t%s\t%s\n" "$id" "$kind" "$code" "$url" "$manifest" > "$results_dir/$i"
  ) &
  pids+=($!)
  i=$((i+1))
  # Limit concurrency
  if (( ${#pids[@]} >= PARALLEL )); then
    wait "${pids[0]}"
    pids=("${pids[@]:1}")
  fi
done < "$INPUT"

# Wait remaining
for pid in "${pids[@]}"; do wait "$pid"; done

# Concatenate ordered results
for f in $(ls "$results_dir" | sort -n); do
  cat "$results_dir/$f" >> "$OUT"
done

rm -rf "$results_dir"
echo "Done. $(wc -l < "$OUT") rows -> $OUT"
