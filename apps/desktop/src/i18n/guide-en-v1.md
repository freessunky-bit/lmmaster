<!-- section: getting-started -->
# Getting Started

When you open LMmaster for the first time, the EULA and a 4-step wizard guide you in. Once the wizard is done, you land on the main screen.

## First-run flow

- **Step 1 — Language**: Pick Korean or English. You can switch later in Settings.
- **Step 2 — PC scan**: We check GPU, RAM, OS so we can suggest models that fit your machine.
- **Step 3 — Runtime install**: Ollama is auto-installed; LM Studio opens its official site.
- **Step 4 — Done**: Install one recommended model right away, or pick from the catalog later.

## Main layout

- The left sidebar holds every feature menu.
- The top bar shows the current screen and the gateway status.
- The gateway port (e.g., 11434) is the address external web apps use to call LMmaster.

## Shortcuts

- **Ctrl+K** (Windows) / **⌘K** (mac) — Command Palette.
- **F1** or **Shift+?** — Keyboard shortcuts help.
- **Ctrl+1–9** — Jump to a menu fast.

---

<!-- section: catalog -->
# Model Catalog

The recommend strip surfaces the top 3 models that fit your PC. Below it, category tabs let you browse the rest.

## Recommendation flow

- Opening the catalog runs a 30-second probe that scores your PC.
- Picks split into **Top quality**, **Balanced**, **Lightweight**.
- Each card has a one-line Korean reason like "fits your PC well".

## Browse by category

- Tabs filter by **summarize**, **translate**, **code**, **chat**, etc.
- Sort by **recommended**, **lowest VRAM**, or **name**.

## Install a model

- Click a card to see license, size, quant options.
- Hit "**Install this model**" to jump to the install center.
- The install center shows progress, ETA, and speed.

## Custom models

- Models you build in the Workbench show up under "**My models**" at the top.
- Click them like any other card to view details and registry info.

---

<!-- section: chat -->
# Chat with your model

Verify a downloaded model right inside LMmaster — and learn how to call the same model from external tools (Ollama CLI, LM Studio).

## In-app chat

- Click **Chat** in the sidebar. Downloaded Ollama models appear in the dropdown automatically.
- Press **Enter** to send. **Shift+Enter** inserts a newline.
- Tokens stream in. When done, the elapsed time (e.g. `2.4s`) is shown.
- Multi-turn history is included automatically. Click **Start over** to clear.
- Auto-scroll pauses while you read older messages. New replies surface a **To bottom ↓** button.

## External tools

LMmaster stores models inside the Ollama daemon (`%USERPROFILE%\.ollama\models` / `~/.ollama/models`).

### 1) Ollama CLI

```bash
ollama list
ollama run sam860/exaone-4.0:1.2b
ollama run sam860/exaone-4.0:1.2b "Hello in Korean please"

# OpenAI-compatible endpoint (for external apps)
curl http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model":"sam860/exaone-4.0:1.2b","messages":[{"role":"user","content":"hi"}]}'
```

- LMmaster's **Local API** keys can be used to route through the LMmaster gateway, which also applies your Pipelines.
- Ollama keeps the model in memory for 5 minutes. First call is slow, subsequent calls are fast.

### 2) LM Studio desktop app

LM Studio uses its own GGUF folder. Easiest path: **download the same model directly inside LM Studio** — Ollama and LM Studio keep models separately. Advanced users can symlink `~/.ollama/models/blobs/sha256-<digest>` into LM Studio's folder.

### 3) llama.cpp / koboldcpp

```bash
./llama-server -m ~/.ollama/models/blobs/sha256-<digest> -c 4096 -ngl 99
# then http://localhost:8080 exposes an OpenAI-compatible endpoint
```

---

<!-- section: model-tuning -->
# Per-model recommended settings

Same model, different parameters → very different output quality. Here are curated recommendations.

## Common parameters

| Parameter | What it controls | Recommended |
|---|---|---|
| **temperature** | Creativity vs consistency | Factual 0.2–0.5 / Chat 0.7 / Creative 0.85–1.0 |
| **top_p** | Token pool | 0.9–0.95 |
| **top_k** | Token count | 40 |
| **repeat_penalty** | Anti-repeat | 1.05–1.15 (Korean particles repeat often → 1.1) |
| **num_ctx** | Context length | 4096 safe / 8192 for long Korean docs |

## Per-model (via Ollama)

### EXAONE 4.0 1.2B Instruct (Korean assistant)
- system prompt: `You are a precise, friendly Korean AI assistant. Say "I don't know" honestly.`
- temperature 0.7 / top_p 0.95 / repeat_penalty 1.1 / num_ctx 8192
- Use the `sam860/exaone-4.0:1.2b` Ollama Hub wrapper (chat template baked in).

### EXAONE 3.5 7.8B Instruct (Korean reasoning)
- temperature 0.6 / top_p 0.95 / repeat_penalty 1.1 / num_ctx 8192
- Strong Korean reasoning + summarization. Weak coding.

### Qwen 2.5 Coder 3B Instruct (coding)
- system prompt: `You are an expert software engineer. Use fenced code blocks.`
- temperature 0.2–0.4 / top_p 0.9 / repeat_penalty 1.05 / num_ctx 8192–16384

### Llama 3.2 3B Instruct (general lightweight)
- system prompt: `You are a helpful assistant.`
- temperature 0.7 / top_p 0.9 / repeat_penalty 1.1 / num_ctx 8192

### Polyglot-Ko 12.8B (roleplay / Korean base)
- **No instruct template** — use few-shot examples instead of system prompt.
- temperature 0.85 / top_p 0.95 / repeat_penalty 1.15 / num_ctx 4096

### HyperCLOVA X SEED 8B (Naver)
- temperature 0.6 / top_p 0.95 / repeat_penalty 1.1 / num_ctx 8192
- Strong on Korean culture / current events. Check Naver license for commercial use.

## Adjusting from the chat page

LMmaster v1 chat uses defaults. For finer control, run from Ollama CLI:

```
/set parameter temperature 0.4
/set parameter num_ctx 16384
```

Or build a custom Modelfile via the Workbench 5-step flow.

## When something looks off

- **Endless repetition** → bump repeat_penalty to 1.1–1.2.
- **Replies too short** → set num_predict 4096+.
- **Answering in English instead of Korean** → add `Reply in Korean.` to the system prompt.
- **Garbled tokens (`[|user|]` etc.)** → chat template missing in the GGUF. Switch to a Hub wrapper like `sam860/exaone-4.0`.
- **Too slow** → reduce num_ctx (8192 → 4096), pick a smaller model, or enable GPU offload (`-ngl 99`).

---

<!-- section: workbench -->
# Workbench

The Workbench is where you craft and register models in 5 steps: Data → Quantize → LoRA → Validate → Register.

## 5-step flow

- **1) Data**: preview JSONL files for your training set. Korean and English entries both work.
- **2) Quantize**: pick Q4_K_M, Q5_K_M, Q8_0, or FP16. Smaller is faster but lower quality.
- **3) LoRA**: set the epoch count and Korean strength.
- **4) Validate**: score the model on a small eval set. See per-category stats.
- **5) Register**: push to the model registry — your model shows up in the catalog. Ollama Modelfile is generated too.

## Pick a runtime

- **mock**: zero network, fastest checks.
- **ollama**: hits your local Ollama server for real metrics.
- **lm-studio**: calls the LM Studio HTTP server.
- After switching runtime, double-check the base URL.

## Stopping a run

- Hit "**Stop**" mid-run; we clean up safely.
- Resumable artifacts (JSONL, LoRA weights) are kept.
- Old temp files are pruned automatically. You can also clean them manually in Settings.

---

<!-- section: knowledge -->
# Knowledge Indexing (RAG)

Inside a workspace, the **Knowledge** tab lets you teach documents to the model. RAG retrieves relevant chunks and stitches them into the model context.

## Ingest

- Provide an absolute path (e.g., `C:/Users/me/notes`) and pick **single file** or **directory (recursive)**.
- Hit "**Start ingest**"; reading → chunking → embedding → writing runs automatically.
- One ingest at a time per workspace.

## Search

- Type a query and choose how many results (1–20).
- Hits come back ranked by cosine similarity.
- Each hit shows the source path, so tracing the origin is easy.

## Workspace isolation

- Each workspace only searches its own data.
- Switching workspace in the sidebar swaps the dataset too.
- Workspaces back up and migrate as a unit (see "Portable Move").

---

<!-- section: api-keys -->
# API Keys + External Apps

LMmaster runs an OpenAI-compatible gateway. External apps just point their base URL at the local LMmaster address.

## Issue a key

- Go to "**Local API**" and hit "**Create key**".
- The plaintext key shows up exactly once — copy it somewhere safe.
- Once dismissed, you'll need to issue a new key.

## Key scope

- **Allowed models**: which models the key can call.
- **Allowed origins**: domains that pass CORS.
- **Expiry**: optional auto-expiry. No-expiry is also allowed.

## Connect an external app

- Set the app's base URL to `http://127.0.0.1:<port>`.
- Pass the key in `Authorization: Bearer sk-lm-...`.
- Model name accepts OpenAI-style aliases (`gpt-4o-mini`) or LMmaster registry IDs.

## Revoke a key

- The "Projects" menu shows who used which key when.
- Suspicious keys can be revoked instantly via "**Revoke**".

---

<!-- section: portable -->
# Portable Move

Export an entire workspace as a single zip and import it on another PC. This delivers ADR-0009's "portable workspace" promise to users.

## Export

- Settings → Portable → "**Move to another PC**".
- Pick options:
  - **Include models**: off = metadata only (a few MB); on = models too (several GB).
  - **Include keys**: off = safer; on = wraps with a passphrase you set.
- Watch progress, ETA, and the sha256 hash.

## Import

- Hit "**Import to this PC**" and pick a zip.
- Preview the manifest first (when, where it was made).
- A zip from a different OS family will need its models re-downloaded; the fingerprint repair tier guides you.

## Safety

- Export halts on integrity errors and cleans up temp files.
- LMmaster never stores your key passphrase. Lose it and the keys can't be recovered.
- USB or cloud sync both work for the move. Same-OS, same-arch is supported.

---

<!-- section: diagnostics -->
# Diagnostics + Auto-Update

Diagnostics shows your PC and LMmaster state at a glance. Settings has the auto-update controls.

## Self-scan

- The diagnostics view shows GPU, VRAM, RAM, disk free.
- A self-scan runs once at startup and on the interval set in Settings.
- The summary is short Korean prose. Open "**Show details**" for raw logs.

## Gateway diagnostics

- See current port, response time, and last request.
- Useful for double-checking that an external app connects.

## Auto-update

- Every 6 hours we check GitHub Releases (the only outbound traffic).
- A toast in the lower-right tells you when a new build is out.
- "**Skip this version**" hides it permanently.
- You can disable auto-update entirely in Settings.

---

<!-- section: faq -->
# FAQ

## My model is slow

- Low VRAM? Try a smaller quant like Q4_K_M.
- The model may have fallen back to CPU. Check the catalog card's recommended VRAM.

## The gateway port keeps changing

- Another process probably grabbed the port; LMmaster picks the next one.
- Pin a port in Settings if available.

## Search returns nothing

- The workspace might be empty. Ingest a folder under "Knowledge".
- The data may be in a different workspace. Check the sidebar switcher.

## Portable move fails

- Check disk free space — zips with models can be several GB.
- Make sure the target path is writable.

## External app gets 401

- The key may have expired or been revoked. Issue a new one under "Local API".
- Make sure the app's domain is in **Allowed origins**.

## Shortcut table

- **Ctrl+K** / **⌘K** — Command Palette
- **F1** / **Shift+?** — Shortcuts help
- **Ctrl+1–9** / **⌘1–9** — Jump to menu
- **Esc** — Close modal, drawer, palette
