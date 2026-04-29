[한국어](./README.md) · [English](./README.en.md)

# LMmaster

[![CI](https://github.com/freessunky-bit/lmmaster/actions/workflows/ci.yml/badge.svg)](https://github.com/freessunky-bit/lmmaster/actions/workflows/ci.yml)
[![Release](https://github.com/freessunky-bit/lmmaster/actions/workflows/release.yml/badge.svg)](https://github.com/freessunky-bit/lmmaster/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-blue.svg)](#license)
[![GitHub Releases](https://img.shields.io/github/v/release/freessunky-bit/lmmaster?include_prerelease&label=release)](https://github.com/freessunky-bit/lmmaster/releases)

> A Korean-first desktop **Local AI Companion** — not a launcher. Existing HTML web apps can keep their UI and just call LMmaster over a local HTTP API or the JS/TS SDK.

LMmaster runs on the user's PC and exposes a local OpenAI-compatible gateway. Existing web apps add one provider, and they get LLM/STT/TTS inference handled by LMmaster: install, update, health check, routing, fallback, key management, and hardware-fit recommendations.

## The 6-pillar promise

LMmaster's v1 commits to six pillars:

1. **Auto install** — A Korean-language wizard installs LM Studio / Ollama and tidies environment variables in one go.
2. **Korean-first** — UI, docs, and error messages all default to Korean (`해요체`). English is one toggle away.
3. **Portable workspace** — Move the workspace folder across machines of the same OS+arch family. Auto-detects and recovers from environment changes.
4. **Curated catalog** — Vetted model manifests under `manifests/snapshot/models/` with Korean descriptions.
5. **Workbench** — Python sidecar scaffold for evaluation / fine-tuning. v1 ships as a placeholder; v1.x expands it.
6. **Self-scan + auto-update** — A 6-hour cron checks environment + catalog. The only outbound traffic is GitHub Releases (telemetry is a separate opt-in toggle).

## Highlights

- **Drop-in for existing web apps** — Add one dependency (`@lmmaster/sdk`) and one provider, done.
- **Hardware probe + deterministic recommender** — Best / balanced / lightweight / fallback selections derived from your hardware, not stochastic LLM output.
- **Multi-runtime** — Adapter-based: llama.cpp · KoboldCpp · Ollama · LM Studio · vLLM. No single-vendor lock-in.
- **OpenAI-compatible local gateway** — Existing OpenAI SDKs work by changing the base URL only.
- **Local API keys with scopes** — Issue scoped keys to other web apps that want to talk to LMmaster.
- **Dark + neon-green design system** — Shared with the existing web apps.

## Quick start

### Users (beta downloads)

> v1 is pre-release. Beta builds are on [GitHub Releases](https://github.com/freessunky-bit/lmmaster/releases).

- **Windows**: Download the `.exe`. If SmartScreen warns, click "More info" → "Run anyway". *(v1 ships unsigned while we build reputation. The warning fades as downloads accumulate.)*
- **macOS**: Download the `.dmg`, drag to Applications, then right-click → "Open" on first launch to pass Gatekeeper.
- **Linux**: Make the AppImage executable and run it.
  ```bash
  chmod +x LMmaster_*.AppImage
  ./LMmaster_*.AppImage
  ```

### Developers

> Prerequisites: Rust (stable), Node 20+, pnpm 9+, Tauri OS prerequisites — see <https://tauri.app/start/prerequisites/>.

```bash
# Install dependencies
pnpm install
cargo build --workspace

# Desktop dev mode
pnpm --filter @lmmaster/desktop tauri dev

# Verification (CLAUDE.md §3 canonical commands)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --exclude lmmaster-desktop
pnpm exec tsc -b
```

## User-controlled toggles (v1)

LMmaster does not phone home without consent. The following 6 controls are all opt-in:

1. **Self-scan interval** — off / 15 min / 60 min (default 60 min).
2. **Auto-update** — 6-hour GitHub Releases cron, default off. Beta-channel toggle is also available.
3. **Anonymous usage telemetry** — default off. When opted in, an anonymous UUID is issued and events go to a GlitchTip self-hosted endpoint (disabled if the DSN env var is not set).
4. **Gemini Korean assistant** — default off. Used only for natural-language install guidance; ranking and recommendations stay deterministic.
5. **Portable export/import** — explicit click only.
6. **Local API key issuance** — scoped keys for other web apps that want to call the LMmaster gateway.

## Repository layout

```text
LMmaster/
├─ apps/desktop/                       # Tauri 2 + React desktop app
├─ crates/                             # Rust core (gateway, runtime, hardware probe, registry, key, ...)
├─ packages/
│  ├─ design-system/                   # Shared tokens/components (desktop + web apps)
│  └─ js-sdk/                          # @lmmaster/sdk
├─ workers/ml/                         # Python ML workbench (v1 placeholder)
├─ manifests/models/                   # Curated model manifest seed
├─ examples/webapp-local-provider/     # Existing-web-app integration demo
└─ docs/                               # ADRs, architecture notes, Korean & dev guides
```

Details: `docs/architecture/03-repo-tree.md`.

## Documentation

Core artefacts:

- Architecture overview — `docs/architecture/00-overview.md`
- Why a companion (not a launcher) — `docs/architecture/01-rationale-companion.md`
- Roadmap (M0–M6) — `docs/architecture/02-roadmap.md`
- Repo layout — `docs/architecture/03-repo-tree.md`
- ADR index — `docs/adr/README.md`
- Risks + mitigations — `docs/risks.md`
- OSS dependency matrix — `docs/oss-dependencies.md`
- Design-system rollout plan — `docs/design-system-plan.md`

Korean user & developer guides:

- Getting started — `docs/guides-ko/getting-started.md` *(planned)*
- Installing models — `docs/guides-ko/install-models.md` *(planned)*
- Existing web app integration — `docs/guides-ko/webapp-integration.md` *(planned)*
- Local API key issuance — `docs/guides-ko/api-keys.md` *(planned)*
- Troubleshooting — `docs/guides-ko/troubleshooting.md` *(planned)*
- UI information architecture — `docs/guides-ko/ui-ia.md`
- SDK integration (devs) — `docs/guides-dev/sdk-integration.md` *(planned)*
- Adapter authoring (devs) — `docs/guides-dev/adapter-authoring.md` *(planned)*

## Things we deliberately do not do

- Build a new inference engine. We adapt mature OSS instead.
- Ship as a browser-only web app. LMmaster is a desktop program.
- Rewrite existing web apps. We add a companion provider.
- Treat training as a v1 core. The workbench is a placeholder.
- Tie ourselves to Ollama. It's one adapter among many.
- Build a light theme.
- Send telemetry to third-party SaaS like Sentry. (See ADR-0041 — GlitchTip self-hosted opt-in only.)

## Contributing

Issues and PRs are welcome. Templates:

- [Bug report](./.github/ISSUE_TEMPLATE/bug_report.md)
- [Feature request](./.github/ISSUE_TEMPLATE/feature_request.md)
- [PR checklist](./.github/PULL_REQUEST_TEMPLATE.md)
- Release automation guide — [.github/SECRETS_SETUP.md](./.github/SECRETS_SETUP.md)

## License

To be finalised. (Our own code is being evaluated as MIT/Apache-2.0 dual. The external OSS license matrix lives in `docs/oss-dependencies.md`.)
