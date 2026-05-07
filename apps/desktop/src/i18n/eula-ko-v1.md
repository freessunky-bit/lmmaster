# LMmaster 사용자 동의서 (v1.2.0)

**적용 시점**: TODO — 출시일 확정 후 채워 주세요.

## 1. 개요

LMmaster는 사용자의 PC에서 로컬로 동작하는 AI 운영 허브예요. 본 동의서는 LMmaster 사용에 관한 사용자 권리·의무를 정의해요.

본 동의서는 LMmaster 본체에만 적용돼요. LM Studio, Ollama, 모델 가중치, Python 의존성 같은 외부 구성 요소는 각 벤더의 라이선스·EULA가 별도로 적용돼요.

## 2. 사용 권한

- 개인적·상업적 용도 모두 사용 가능해요.
- 코드 수정·재배포는 LICENSE 파일의 조건 아래 가능해요.
- 본 앱을 활용해 만든 결과물은 사용자 본인 소유예요.

## 3. 외부 통신

- LMmaster 본체는 외부 통신 0 원칙(ADR-0013)을 따라요.
- 자동 업데이트 확인은 GitHub Releases (api.github.com) 1회/6시간 — 사용자가 Settings에서 비활성 가능해요.
- LM Studio · Ollama 같은 외부 런타임은 각 벤더의 EULA가 별도 적용돼요. 우리가 그 EULA를 다시 보여드리지 않아요.
- 텔레메트리 opt-in을 켰을 때만 익명 통계가 단일 endpoint(GlitchTip self-hosted)로 전송돼요.

## 4. 데이터

- 사용자가 입력한 프롬프트·문서·모델은 모두 PC에서만 처리돼요.
- 워크스페이스 데이터는 portable 디렉터리(`%APPDATA%\LMmaster` 등)에 저장돼요.
- 텔레메트리는 기본 비활성. opt-in했을 때만 익명 PC 통계(OS major version / GPU 모델 / VRAM)가 전송돼요. 프롬프트 / 모델 출력 / 파일 내용은 절대 전송되지 않아요.

## 5. 책임 한계

TODO — 법무 검토 후 채워 주세요. 표준 disclaimer 권장.

기본 골자(작성 가이드):
- 본 소프트웨어는 "있는 그대로(AS IS)" 제공돼요. 명시적·묵시적 보증을 제공하지 않아요.
- LMmaster 사용 중 발생한 데이터 손실 / 모델 출력 부정확성 / 외부 런타임 장애에 대해 LMmaster 측은 책임을 지지 않아요.
- 사용자가 외부에 공개·배포하는 모델 출력은 사용자 본인의 책임이에요.

## 6. 변경

본 동의서가 변경되면 사용자에게 알리고 다시 동의를 요청해요.

- patch 버전(예: 1.0.1)은 자동 동의로 갈음해요 (오타 / 명확화).
- minor / major(예: 1.1.0 / 2.0.0)는 재동의가 필요해요 (기능 / 데이터 처리 변경).
- 갱신 시 변경 요약을 함께 보여드려요.

## 7. NSFW 데이터셋 정책 (v0.1.0+)

LMmaster 데이터셋 카탈로그(Phase 23')는 다음 정책을 따라요:

1. **미성년자 콘텐츠 금지** — 큐레이터가 자동 키워드 scan(영문/일본어/한국어) + 본문 검토로 1차 거부해요. 사용자가 미성년 묘사 데이터를 발견하면 즉시 신고해 주세요.
2. **HF NFAA 플래그 필수** — Not-For-All-Audiences 플래그가 없는 NSFW 데이터셋은 카탈로그에 등록되지 않아요.
3. **라이선스 화이트리스트** — Apache-2.0 / MIT / OpenRAIL-M / CC-BY 등 검증된 오픈 라이선스만. CC-BY-NC는 *비상업 사용자 동의* 후만 노출돼요.
4. **사용자 책임** — 다운로드 후 사용은 사용자 PC 안에서만 일어나요. 사용자는 본인 국가의 법(한국 청소년보호법 / 미국 PROTECT Act / EU CSAM regulation 등)을 준수할 의무가 있어요.
5. **NSFW 토글** — 카탈로그 헤더의 3-state 토글로 *숨김 / 모두 보임 / 성인만* 사이클 선택 가능해요. 기본은 *숨김*이에요. 모델 NSFW와 데이터셋 NSFW 게이트가 통합돼 있어요.

자세한 정책: `docs/adr/0062-nsfw-dataset-policy.md` 참조.

## 9. AI 트렌드 리포트 정책 (v1.2.0+, Phase 22')

LMmaster의 *AI 트렌드 리포트* 메뉴(별도 4B+ 모델 설치 시 활성)는 다음 정책을 따라요:

1. **외부 트렌드 데이터셋 fetch 동의** — 사용자 PC가 `cdn.jsdelivr.net`에서 큐레이션된
   `trends-bundle.json`을 매주 1회 fetch해요. 사용자 PC가 직접 RSS / SNS / 뉴스 사이트를
   scrape하지 않아요 (외부 통신 0 정체성 보존).
2. **큐레이터 흐름** — 별도 repo `lmmaster-trends-bundle`(또는 본 repo prototype)의
   GHA aggregator가 RSS / arXiv / HF Daily Papers / YouTube / Bluesky / Mastodon
   소스를 사람이 검토 후 fair-use 기준 한국어 한 줄 요약으로 변환해 push해요.
3. **로컬 LLM 한국어 요약 정책** — 메뉴 진입 시 사용자 PC의 4B+ 모델(Gemma 3 4B /
   Nemotron 3 Nano 4B / EXAONE 3.5 7.8B / HCX-SEED 8B 등)이 카테고리별로 1~2문장
   해요체 메타 요약을 생성해요. *원문 재출판은 금지*예요. 결과는 30일 캐시돼요.
4. **저작권자 신고 채널** — 본인이 만든 콘텐츠가 부당하게 인용됐다고 판단하시면
   본 repo의 GitHub Issue로 신고해 주세요. 큐레이터가 1주 안에 검토해 다음 push에서
   제외하거나 인용 형식을 조정해요.

자세한 정책: `docs/adr/0060-trend-report.md` + `docs/research/phase-22p-trend-report-decision.md` 참조.

## 10. 문의

- 공식 이메일: wind@joycity.com
- 본 동의서에 동의하지 않으시면 "동의하지 않을래요"를 눌러 앱을 종료할 수 있어요.
