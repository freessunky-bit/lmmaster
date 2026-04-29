# 컨트리뷰션 가이드

## 시작 전 읽을 것

- `docs/architecture/00-overview.md`
- `docs/architecture/01-rationale-companion.md`
- `docs/adr/README.md` 와 ADR-0001 ~ ADR-0014

## 핵심 원칙

- 코어 스택 변경은 **반드시 ADR**.
- companion 경계 침범 금지: 기존 웹앱은 호출만, runtime 직접 의존 금지.
- 클래스명에 `ad-/adv-/ads-/banner-/sponsor-` 사용 금지.
- 라이트 테마 만들지 않는다.
- 한국어 카피는 `voice.md` 톤 준수.

## PR 체크리스트

- [ ] 이 변경이 ADR을 새로 추가하거나 수정해야 하는가?
- [ ] 의존 방향 규칙(ADR-0004의 단방향)을 깨지 않는가?
- [ ] 한국어 카피가 voice.md 톤을 따르는가?
- [ ] 라이선스 매트릭스에 영향이 있는가?
- [ ] 테스트 추가/갱신했는가?

## 빌드

```bash
pnpm install
cargo build --workspace
pnpm --filter @lmmaster/desktop tauri dev
```
