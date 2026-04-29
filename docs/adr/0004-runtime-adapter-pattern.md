# ADR-0004: 런타임 어댑터 패턴 + RuntimeAdapter trait

- Status: Accepted
- Date: 2026-04-26

## Context
지원 대상 런타임이 다양하다 — llama.cpp, KoboldCpp, vLLM, Ollama, LM Studio. 각자 설치 방식, 모델 포맷, HTTP API, capability(비전/툴/구조화 출력/임베딩)가 다르다. 런타임을 하드코딩하면 추가/교체가 어려워진다.

## Decision
모든 런타임은 단일 `RuntimeAdapter` trait을 구현한다. Runtime Manager는 trait 객체만 본다. 새로운 런타임 추가는 새 crate + trait 구현 1개.

trait의 최소 메서드:
```rust
trait RuntimeAdapter: Send + Sync {
    async fn detect(&self) -> DetectResult;
    async fn install(&self, opts: InstallOpts) -> Result<()>;
    async fn update(&self) -> Result<()>;
    async fn start(&self, cfg: RuntimeCfg) -> Result<RuntimeHandle>;
    async fn stop(&self, h: &RuntimeHandle) -> Result<()>;
    async fn restart(&self, h: &RuntimeHandle) -> Result<()>;
    async fn health(&self, h: &RuntimeHandle) -> HealthReport;
    async fn list_models(&self) -> Vec<LocalModel>;
    async fn pull_model(&self, m: &ModelRef, sink: ProgressSink) -> Result<()>;
    async fn remove_model(&self, m: &ModelRef) -> Result<()>;
    async fn warmup(&self, h: &RuntimeHandle, m: &ModelRef) -> Result<()>;
    fn capability_matrix(&self) -> CapabilityMatrix; // vision/tools/struct/embed
    fn serving_endpoints(&self, h: &RuntimeHandle) -> Endpoints; // 내부용
}
```

## Consequences
- 런타임 차이가 어댑터 안에서만 처리된다.
- gateway는 모델→런타임 매칭 + 오케스트레이션에만 집중.
- capability_matrix를 통해 gateway가 "이 모델이 tool calling 가능한지"를 결정.
- 각 어댑터가 별도 crate로 분리되면 빌드 시간이 늘 수 있음 — workspace + cargo profile로 완화.

## Alternatives considered
- **하드코딩된 if-else**: 거부.
- **dynamic plugin (libloading)**: v2 옵션. v1은 컴파일 타임 등록으로 충분.

## References
- ADR-0005 (llama.cpp 우선)
