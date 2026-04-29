# Phase 4.d + 4.g — Projects 화면 + Settings 화면 결정 노트 (lightweight)

> 작성일: 2026-04-27
> 상태: 구현 완료
> 선행: Phase 4.a (StatusPill / VirtualList), Phase 3' (게이트웨이 + ApiKeysPanel + WorkspaceRepairBanner), Phase 1A (마법사)
> 후행: Phase 5' (워크벤치 v1 — settings advanced toggle 활성), Phase 6' (Gemini opt-in / STT-TTS 활성), v1.1 (settings 다중 워크스페이스 / 진단 로그 export)

---

## 0. 결정 요약

1. **Projects = ApiKeysPanel 데이터 source 재사용 + alias prefix 그룹화 dashboard**. CRUD UI(`ApiKeysPanel`)와는 navigation 분리.
2. **Settings = 4 카테고리 nav (일반 / 워크스페이스 / 카탈로그 / 고급) + 우측 form 패널**. localStorage 기반 클라이언트 settings.
3. **사용량 차트 mock**: 게이트웨이 access log IPC가 v1에 없으므로 `mockSparkline(group_id)` deterministic seed로 24개 sample. `// TODO Phase 6': real access log IPC` 주석. 그룹 id 변경 없으면 매 렌더 같은 그래프.
4. **localStorage 키 네이밍**: `lmmaster.settings.{section}.{key}` 형식. v1.1에 Tauri store / portable workspace로 이동.
5. **테마 light는 disabled** + lock 아이콘 + "곧 만나요" 텍스트. Dark-only 정책 (디자인 시스템 contract).
6. **registry URL은 read-only**. 외부 통신 0 정책 — 사용자 변경 불가.
7. **Gemini / STT-TTS / 진단 로그 export / 워크스페이스 relocate / 카탈로그 refresh** 모두 disabled placeholder. 각 항목에 `comingSoon` 텍스트.

---

## 1. 채택안

### 1.1 Projects 화면

- **IA**: `<topbar: 활성 카운트>` + 우측 카드 그리드 (CSS grid auto-fit minmax(240px, 1fr)) + 선택 시 우측 drawer.
- **그룹화 함수**: `aliasPrefix(alias)` = trim + `split(/\s+/)`의 첫 토큰. "내 블로그 메인", "내 블로그 미러" → 같은 prefix "내".
- **카드 구성**: header(displayName + StatusPill is-active|is-dim) + body(origin chips top 3 + meta dl: 허용 모델 / 키 수 / 마지막 사용) + footer("사용량 보기" 버튼).
- **drawer 구성**: header(displayName + close x) + body(24h sparkline mock + total requests + top 3 models bar + 키 목록 with revoke). Esc 키로 닫힘.
- **revoke 흐름**: confirm → `revokeApiKey` → refresh → drawer 안 키 row가 dim 표시로 갱신.
- **빈 상태**: "아직 발급한 키가 없어요" + "API 키로 가볼게요" CTA (anchor). v1 단순 hash anchor — 메인 shell이 hash listen.
- **a11y**: 카드는 article + aria-labelledby. drawer는 dialog + aria-modal + Esc 닫기. sparkline은 `<svg role="img" aria-label="…횟수">`.

### 1.2 Settings 화면

- **IA**: `<topbar: 빌드 버전 + 마지막 확인 시각>` + 좌측 nav (4 카테고리, sm 240px) + 우측 form 패널.
- **일반**: 언어 / 테마(dark only) / 자가스캔 주기 / 음성(disabled).
- **워크스페이스**: 경로(read-only) / relocate(disabled) / "지금 정리할게요" 버튼 → `checkWorkspaceRepair` 호출 → 결과 메시지.
- **카탈로그**: registry URL(read-only) / 마지막 갱신 / "지금 갱신"(disabled).
- **고급**: Gemini(disabled) / SQLCipher env hint / 진단 로그 export(disabled) / 빌드 정보.
- **localStorage 헬퍼** (`ipc/settings.ts`): `getScanInterval` / `setScanInterval` / `getNotifyOnPhase5` / `setNotifyOnPhase5` / `getEncryptDbHint` (read-only placeholder).
- **a11y**: 각 form은 `<form>` + 카테고리는 `<fieldset><legend>`. 라디오는 `role="radio"` (button 패턴), 토글은 `role="switch" aria-checked`. visually-hidden h3로 form 제목.

---

## 2. 기각안 + 이유 (negative space — 의무)

### 2.1 ApiKeysPanel을 projects 화면에 흡수

- **시도 / 검토 내용**: ApiKeysPanel(keys 화면)이 이미 키 발급 + 회수 UI를 갖고 있으니 projects를 합쳐 nav 한 개 줄이기.
- **거부 이유**: keys = "키 발급·회수 액션" 중심 / projects = "사용량 분석 + 묶어 보기" 분석 중심. 액션과 분석이 같은 페이지에 섞이면 사용량 차트 위에 회수 버튼이 보이는 등 잘못 회수 위험. navigation 분리 + cross-link로 충분.
- **재검토 트리거**: 사용자 telemetry로 keys → projects 이동 빈도 90%+ 면 통합 검토.

### 2.2 Settings에 키 발급 추가 (single source of truth 위반)

- **시도 / 검토 내용**: settings는 일반적으로 모든 환경 설정의 진입점이라 키 입력란을 추가 가능.
- **거부 이유**: 키 발급 + 회수의 single source of truth = ApiKeysPanel. settings에 추가하면 두 곳에서 입력 가능 → UX 혼란 + 보안 audit log 분기 위험. settings는 link만, 발급은 keys 화면.
- **재검토 트리거**: 없음 (의도적 분리).

### 2.3 sparkline 차트 라이브러리 (chart.js / recharts) 도입

- **시도 / 검토 내용**: chart.js / recharts는 풍부한 시각화 + 인터랙션 지원.
- **거부 이유**: 24개 점 sparkline은 SVG `<rect>` 24개로 충분 — 50KB+ 의존성을 추가할 가치 없음. v1.1 7d/30d 로 확장하면서 dynamic 인터랙션이 필요해지면 그때 검토. v1은 inline SVG로 zero-deps.
- **재검토 트리거**: drawer에 hover tooltip / 시간대별 axis label / per-model breakdown 차트가 필요해질 때.

### 2.4 Gemini API 키 입력란 추가 (settings 고급)

- **시도 / 검토 내용**: Phase 6'에서 Gemini opt-in을 다루기 전, settings 고급에 자리만 미리 마련.
- **거부 이유**: API 키 입력은 단순 form이 아니라 권한 / 사용 정책 / disclaimer / opt-in 흐름이 필요. Phase 6' ADR 후 통합 출시가 안전. 또한 v1 GUI 직접 입력은 키 plaintext 노출 위험 — Phase 6' 진입 전에는 env 변수로만 접근, v1.1+ 검토.
- **재검토 트리거**: Phase 6' 진입 시.

### 2.5 워크스페이스 다중 인스턴스

- **시도 / 검토 내용**: 사용자가 여러 워크스페이스를 동시에 운영 (예: 회사 PC / 노트북 / 외장하드).
- **거부 이유**: portable-workspace crate 자체가 단일 root 가정. multi-instance는 크로스 SQLite + lock + 동기화 등 별도 설계 필요. v2 검토.
- **재검토 트리거**: v2 진입 시.

### 2.6 카탈로그 registry URL 사용자 입력

- **시도 / 검토 내용**: 사용자가 자체 registry mirror를 가리킬 수 있도록 입력란 노출.
- **거부 이유**: 외부 통신 0 정책상 v1 registry는 동봉된 read-only 매니페스트. mirror 도입은 manifest 검증 + 신뢰 체인 ADR 필요. v1.1+ 검토.
- **재검토 트리거**: 사용자 피드백 또는 enterprise 고객 미러 요청 시.

---

## 3. 미정 / 후순위

| 항목 | 이월 사유 | 페이즈 |
|---|---|---|
| Projects 7d/30d sparkline | 24h 표시로 v1 충분 | v1.1 |
| Projects 사용량 IPC (real access log) | Phase 3' 게이트웨이 access log 테이블에 의존 | Phase 6' |
| Settings 다중 워크스페이스 | crate 단일 root 가정 | v2 |
| Settings 진단 로그 export (.zip) | bug report 채널 미정 | v1.1 |
| Settings 카탈로그 refresh | registry-fetcher IPC 신규 wire-up | v1.1 |
| Settings 테마 light | 디자인 시스템 light 토큰 작성 후 | v1.x |
| Settings Gemini opt-in 동작 | Phase 6' Gemini ADR | Phase 6' |
| Settings STT/TTS 토글 동작 | 모델 라이선스 검토 | Phase 6' |
| Settings 워크스페이스 relocate | Tauri shell open + workspace migration | v1.1 |
| Projects 카드 팀원 슬롯 | team mode 전체 | Phase 6' team |

---

## 4. 테스트 invariant

- **Projects 그룹화**: 같은 alias prefix의 키들이 한 카드. 다른 prefix는 다른 카드.
- **Projects 빈 상태**: keys=[]일 때 empty title + body + CTA.
- **Projects revoke**: drawer revoke 버튼 → confirm → revokeApiKey 호출. confirm reject 시 호출 안 함.
- **Projects revoked 카드**: 모든 키가 revoked인 그룹은 `is-dim` 클래스 + StatusPill `idle`.
- **Projects drawer Esc**: Esc 키로 drawer 닫힘.
- **Projects sparkline deterministic**: 같은 group id로 같은 sparkline 반환.
- **Settings 카테고리 nav**: 4 카테고리 모두 렌더 + radiogroup + 선택 시 panel 변경.
- **Settings localStorage 갱신**: 자가스캔 주기 라디오 클릭 시 `lmmaster.settings.general.scan_interval_min` 갱신.
- **Settings 언어 라디오**: i18n.changeLanguage('en') 호출.
- **Settings light 테마**: input.disabled = true.
- **Settings registry URL**: input.readOnly = true.
- **Settings advanced toggles**: Gemini / 로그 export 모두 disabled.
- **Settings repair 버튼**: 클릭 시 `checkWorkspaceRepair` 호출 + 성공 메시지.
- **a11y 0 violation**: axe-core run on Projects + Settings (region 룰은 App shell 책임이라 disabled).

---

## 5. 다음 페이즈 인계

- Phase 4 통합 시 `App.tsx`에 `activeNav==='projects'` → `<Projects />`, `activeNav==='settings'` → `<Settings />` 분기 추가.
- Phase 4.h Korean preset 작업 시 settings 고급에 "기본 preset 카테고리" 라디오 추가 검토 (v1은 mock 또는 catalog 화면 자체 노출).
- Phase 6' Gemini ADR 진입 시 settings 고급 toggle을 enabled로 변경 + 동작 wire-up.

---

## 6. 참고

- **Linear / Vercel dashboard** — projects 카드 그리드 + alias + origin chip 패턴 차용.
- **macOS System Preferences / GNOME Settings** — 4 카테고리 nav + radio + form 패턴.
- **Toss UX 8원칙** — "곧 만나요" / "다음 업데이트에서 만나요" 같은 회복 액션 동반.
- **ADR-0006** (디자인 시스템) — 모든 토큰 사용. 인라인 색 / 여백 0.
- **ADR-0022** (게이트웨이 + scoped key) — projects 화면이 access log 소비. read-only.

---

**문서 버전**: v1.0 (2026-04-27 구현 완료).
