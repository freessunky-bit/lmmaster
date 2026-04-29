# ADR-0003: Native core/gateway 언어로 Rust + Axum 채택

- Status: Accepted
- Date: 2026-04-26

## Context
Local Gateway, Runtime Manager, Hardware Probe, Portable Workspace, Key Manager는 (1) 자식 프로세스 supervisor (2) 파일/네트워크 I/O (3) HTTP+SSE 서버 (4) OS native API 호출이 모두 필요하다. Tauri 2의 backend 언어는 Rust.

## Decision
Native 레이어 전체를 Rust로 작성한다. HTTP/SSE는 **Axum**, async runtime은 **tokio**, 직렬화는 **serde**, SQLite 접근은 **sqlx** 또는 **rusqlite**(둘 중 ADR-0008에서 별도 결정).

## Consequences
- Tauri와 같은 언어/툴체인 → 단일 빌드 파이프라인.
- 무거운 supervisor를 안전하게 (ownership/Send/Sync) 작성 가능.
- 팀 학습 곡선이 있다 — 이건 워크벤치(Python) 분리로 일부 우회.
- Rust ↔ Python(워크벤치) 경계는 표준 IPC(stdio JSON-RPC 또는 local socket)로 명확히.

## Alternatives considered
- **Go + Gin**: 생산성 좋지만 Tauri와 언어 분리됨. 거부.
- **Node + Fastify**: 가능하지만 supervisor/하드웨어 probe 신뢰성/성능에서 Rust 대비 불리. 거부.
- **C++**: 메모리 안전 비용. 거부.

## References
- https://github.com/tokio-rs/axum
- ADR-0002 (Tauri 2)
