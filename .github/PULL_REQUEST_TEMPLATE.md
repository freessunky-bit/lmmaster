# Pull Request

> 한국어 해요체 또는 English — 편한 언어로 적어 주세요.

## 무엇을 바꿨나요? / What does this change?

<!-- 한두 문단으로 변경 요지를 알려 주세요. -->

## 왜 바꿨나요? / Why?

<!-- 어떤 사용자 페인 또는 ADR / 결정 노트에 근거했어요? Link 부탁해요. -->

- 관련 이슈: #
- 관련 ADR / 결정 노트: `docs/adr/...` 또는 `docs/research/...`

## 어떻게 검증했나요? / How was it verified?

검증 명령은 CLAUDE.md §3 형식만 사용해 주세요.

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace --exclude lmmaster-desktop`
- [ ] `pnpm exec tsc -b` (TypeScript 변경 시)
- [ ] `pnpm exec vitest run` (UI 변경 시)
- [ ] `pnpm run build` (frontend 빌드 변경 시)

## UI 변경 체크리스트 / UI checklist

(UI 변경이 없으면 비워 두세요.)

- [ ] a11y: vitest-axe `violations.toEqual([])` 통과.
- [ ] 키보드 / Esc / 포커스 이동: §4.3 게이트 준수.
- [ ] 한국어 카피 §4.1 톤 매뉴얼 일관 (해요체 / 영어 노출 0).
- [ ] i18n `ko.json` + `en.json` 동시 갱신.
- [ ] 디자인 토큰만 사용 — 인라인 색·여백·radius 금지.

## 위험 노트 / Risk notes

<!-- 알려진 한계, 후속 작업, 사용자 영향 등을 적어 주세요. -->

## 스크린샷 / Screenshots (optional)

<!-- before/after 비교가 가능하면 첨부해 주세요. -->
