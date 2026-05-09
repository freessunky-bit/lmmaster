# LMmaster End User License Agreement (v1.2.0)

**Effective date**: This agreement takes effect when you accept it for the first time. The acceptance timestamp and the agreement version (v1.2.0) are stored locally on your PC. If a minor / major revision is published, you will be asked to accept the new version (see §6).

This agreement does not limit any consumer rights guaranteed by the mandatory laws of your country of residence (including, where applicable, the Korean consumer-protection statutes). Where any clause conflicts with such mandatory law, that clause is adjusted only to the extent required, and the rest remains in force.

## 1. Overview

LMmaster is a local AI operations hub that runs on your own PC. This agreement defines your rights and obligations when using LMmaster.

This agreement applies only to LMmaster itself. External components such as LM Studio, Ollama, model weights, and Python dependencies are governed by their own licenses and EULAs (see §8).

## 2. Permitted use

- Personal and commercial use are both allowed.
- Code modification and redistribution are governed by the LICENSE file.
- Whatever you produce using this app is yours.

You may not use LMmaster for any of the following purposes. If you do, you bear sole legal and ethical responsibility:

- Generating or distributing content that infringes third-party rights (copyright, trademark, right of publicity, defamation, privacy, trade secrets, etc.).
- Creating, possessing, or distributing sexual or violent content depicting minors (in violation of e.g. the Korean Youth Protection Act and the Act on the Protection of Children and Youth Against Sex Offenses, the US PROTECT Act, the EU CSAM Regulation, and equivalent laws).
- Voice phishing, fraud, hacking, malware, spam, or any other criminal or unfair-competition purpose.
- Promoting or auto-generating discrimination, hate, threats, or harassment against any individual or group.
- Substituting LMmaster output for licensed professional advice in domains that require credentials (medicine, law, finance, etc.).

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

- The software is provided "AS IS" and "AS AVAILABLE". To the maximum extent permitted by applicable law, no warranties of any kind are provided, whether express or implied (including but not limited to merchantability, fitness for a particular purpose, non-infringement, accuracy, uninterrupted operation, or freedom from defects).
- LMmaster is not liable for any of the following arising out of your use of the software:
  - Loss, corruption, or destruction of data on your PC, including workspace files and downloaded model weights.
  - Inaccuracy, bias, hallucination, offensiveness, illegality, or third-party-rights infringement of model outputs.
  - Failures, EULA breaches, or license changes of external runtimes such as LM Studio, Ollama, llama.cpp, or Python.
  - Transient failures of catalog fetch or auto-update checks.
- You are solely responsible for the legality and ethics of any model output you publish, distribute, or use commercially.
- Because LMmaster is distributed free of charge, the aggregate liability of LMmaster for any direct, indirect, special, incidental, consequential, or punitive damages caused by the software is, *to the maximum extent permitted by applicable law*, limited to the amount you actually paid LMmaster for the software (typically zero).
- The above disclaimers and liability caps do not apply to (a) LMmaster's intentional misconduct or gross negligence, or (b) any liability that cannot be excluded or limited under the Korean Act on the Regulation of Terms and Conditions, the Korean Act on the Consumer Protection in Electronic Commerce, or any mandatory law of your country of residence. In those cases the relevant law governs.

## 6. Changes

When this agreement changes, we will notify you and ask you to accept it again.

- Patch versions (e.g. 1.0.1) are auto-accepted (typos / clarifications).
- Minor / major versions (e.g. 1.1.0 / 2.0.0) require re-acceptance (feature / data-handling changes).
- A change summary is shown alongside any update.

## 7. NSFW Dataset Policy (v0.1.0+)

The LMmaster dataset catalog (Phase 23') follows this policy:

1. **No minor content** — Curators automatically scan English/Japanese/Korean keywords and manually review samples to reject minor depictions. Please report any such data you encounter.
2. **HF NFAA flag required** — Datasets without the Not-For-All-Audiences flag are not registered in the NSFW catalog.
3. **License whitelist** — Only Apache-2.0 / MIT / OpenRAIL-M / CC-BY and similar verified open licenses. CC-BY-NC requires explicit non-commercial user consent.
4. **User responsibility** — Downloads and usage stay on the user's PC. You are responsible for complying with your country's laws (Korean Youth Protection Act / US PROTECT Act / EU CSAM regulation, etc.).
5. **NSFW toggle** — The 3-state toggle in the catalog header cycles through *hide / show all / adult only*. Default is *hide*. Model NSFW and dataset NSFW gating are unified.

Full policy: `docs/adr/0062-nsfw-dataset-policy.md`.

## 8. Third-Party Models / Weights / Datasets (v1.0.0+)

Model weights you download or run through LMmaster (e.g. Llama, Qwen, Gemma, EXAONE, HCX-SEED) and the datasets and runtimes they depend on are governed by their own licenses, EULAs, and acceptable-use policies.

- LMmaster makes no representation or warranty about the accuracy, safety, legality, or license compliance of any external model, dataset, or runtime.
- It is your responsibility to read each model's model card and license to determine whether your intended use (including redistribution and commercial use) is permitted. Some models (e.g. Llama Acceptable Use Policy, Gemma Prohibited Use Policy) impose their own usage restrictions.
- It is your responsibility to verify that content generated by these models does not infringe any third-party rights (copyright, right of publicity, defamation, privacy, trade secrets).
- LMmaster is not responsible for network costs, disk usage, or security risks incurred while downloading model weights or datasets.

## 9. AI Trend Report Policy (v1.2.0+, Phase 22')

The LMmaster *AI Trend Report* menu (activated when a 4B+ model is installed) follows this policy:

1. **External trend dataset fetch consent** — Your PC fetches a curated `trends-bundle.json` from `cdn.jsdelivr.net` once per week. Your PC does not directly scrape RSS / SNS / news sites (preserves the zero-external-call identity).
2. **Curator workflow** — A separate repo `lmmaster-trends-bundle` (or in-repo prototype) runs a GHA aggregator that human-reviews RSS / arXiv / HF Daily Papers / YouTube / Bluesky / Mastodon sources and converts them into fair-use-compliant Korean one-line summaries.
3. **Local LLM Korean summary policy** — On menu entry, your PC's 4B+ model (Gemma 3 4B / Nemotron 3 Nano 4B / EXAONE 3.5 7.8B / HCX-SEED 8B, etc.) generates 1–2 sentence meta-summaries per category in Korean. *Republishing the original content is forbidden.* Results are cached for 30 days.
4. **Copyright holder reporting channel** — If you believe your content has been improperly cited, please file a GitHub Issue on this repo. The curator will review within one week and exclude or adjust the citation in the next push.

Full policy: `docs/adr/0060-trend-report.md` + `docs/research/phase-22p-trend-report-decision.md`.

## 10. Contact

- **General questions / bugs / feature requests**: <https://github.com/freessunky-bit/lmmaster/issues>
- **Security vulnerability reports** (private channel): <https://github.com/freessunky-bit/lmmaster/security/advisories/new>
- **Copyright holder reports** (Trends Report citation): please file a GitHub Issue.
- If you do not agree with this agreement, click "I do not agree" to exit the app.

## 11. General provisions

- **Governing law**: This agreement is interpreted and applied under the laws of the Republic of Korea, without prejudice to any consumer rights guaranteed by the mandatory laws of your country of residence.
- **Dispute resolution / jurisdiction**: Disputes arising under this agreement should first be raised on the LMmaster GitHub Issue channel for amicable resolution. If that fails, the court of your domicile (or residence) in the Republic of Korea has exclusive jurisdiction at first instance. Where domicile or residence cannot be determined, jurisdiction follows the Korean Civil Procedure Act.
- **Severability**: If any clause of this agreement is held to be invalid or unenforceable, the remaining clauses remain in full effect.
- **Assignment**: You may not assign your rights or obligations under this agreement to any third party without LMmaster's prior consent.
- **Export controls**: You agree not to export or re-export the software or any model weights in violation of applicable export-control laws of your country of residence (e.g. the Korean Foreign Trade Act, the US Export Administration Regulations).
