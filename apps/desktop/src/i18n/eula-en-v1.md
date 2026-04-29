# LMmaster End User License Agreement (v1.0.0)

**Effective date**: TODO — fill in once the release date is confirmed.

## 1. Overview

LMmaster is a local AI operations hub that runs on your own PC. This agreement defines your rights and obligations when using LMmaster.

This agreement applies only to LMmaster itself. External components such as LM Studio, Ollama, model weights, and Python dependencies are governed by their own licenses and EULAs.

## 2. Permitted use

- Personal and commercial use are both allowed.
- Code modification and redistribution are governed by the LICENSE file.
- Whatever you produce using this app is yours.

## 3. External traffic

- The LMmaster core honors the "zero outbound traffic by default" rule (ADR-0013).
- Auto-update checks hit GitHub Releases (api.github.com) once every 6 hours — you can turn this off in Settings.
- External runtimes such as LM Studio and Ollama have their own EULAs. We do not re-display those.
- Anonymous telemetry is only sent when you opt in. The endpoint is a single self-hosted GlitchTip instance.

## 4. Data

- Your prompts, documents, and models are processed locally on your PC.
- Workspace data lives in a portable directory (e.g. `%APPDATA%\LMmaster`).
- Telemetry is off by default. When you opt in, only anonymous PC stats (OS major version / GPU model / VRAM) are sent. Prompts, model outputs, and file contents are never transmitted.

## 5. Limitation of liability

TODO — fill in after legal review. A standard disclaimer is recommended.

Skeleton (drafting guide):
- The software is provided "AS IS" without warranties of any kind, express or implied.
- LMmaster is not liable for data loss, model output inaccuracies, or external runtime failures encountered while using the app.
- You are responsible for any model outputs you publish or distribute.

## 6. Changes

When this agreement changes, we will notify you and ask you to accept it again.

- Patch versions (e.g. 1.0.1) are auto-accepted (typos / clarifications).
- Minor / major versions (e.g. 1.1.0 / 2.0.0) require re-acceptance (feature / data-handling changes).
- A change summary is shown alongside any update.

## 7. Contact

- Official email: TODO — fill in.
- GitHub Issues: TODO — fill in.
- If you do not agree with this agreement, click "I do not agree" to exit the app.
