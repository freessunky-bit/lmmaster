// @lmmaster/sdk — 기존 웹앱이 LMmaster local gateway를 호출하기 위한 SDK.
//
// 정책 (ADR-0022):
// - OpenAI 호환 baseUrl + apiKey 기반.
// - gateway가 byte-perfect SSE relay — SDK는 단순 SSE 파서.
// - 거부 envelope을 LMmasterApiError로 보존.

export * from "./client";
export * from "./types";
export * from "./discovery";
export * from "./chat";
export * from "./models";
export * from "./install";
export * from "./keys";
export * from "./projects";
