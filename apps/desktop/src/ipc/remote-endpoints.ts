// 원격 LMmaster 게이트웨이 연결 관리 IPC.
// 사용자 B가 사용자 A의 모델을 쓰기 위한 엔드포인트 등록/조회/삭제/테스트.

import { invoke } from "@tauri-apps/api/core";

export interface RemoteEndpoint {
  id: string;
  alias: string;
  /** "/v1" 포함 base URL (예: "http://192.168.1.10:14964/v1"). */
  base_url: string;
  api_key: string;
  created_at: string;
}

/** 채팅 드롭다운에 노출되는 원격 모델 1개. */
export interface RemoteModelInfo {
  /** "remote::{endpoint_id}::{model_id}" 형태 — Chat.tsx selectedRuntimeId로 사용. */
  runtime_id: string;
  endpoint_id: string;
  endpoint_alias: string;
  model_id: string;
  /** "{alias} · {model_id}" 드롭다운 표시명. */
  display_name: string;
}

export type RemoteEndpointError =
  | { kind: "save"; message: string }
  | { kind: "test-failed"; message: string }
  | { kind: "not-found"; id: string }
  | { kind: "internal"; message: string };

/** 저장된 원격 연결 목록. */
export async function listRemoteEndpoints(): Promise<RemoteEndpoint[]> {
  return invoke<RemoteEndpoint[]>("list_remote_endpoints");
}

/** 원격 연결 추가. 성공 시 생성된 엔드포인트 반환. */
export async function addRemoteEndpoint(args: {
  alias: string;
  base_url: string;
  api_key: string;
}): Promise<RemoteEndpoint> {
  return invoke<RemoteEndpoint>("add_remote_endpoint", {
    alias: args.alias,
    baseUrl: args.base_url,
    apiKey: args.api_key,
  });
}

/** 원격 연결 삭제. */
export async function removeRemoteEndpoint(id: string): Promise<void> {
  return invoke<void>("remove_remote_endpoint", { id });
}

/**
 * 연결 테스트 — /v1/models 호출.
 * 성공: 사용 가능한 model_id 목록. 실패: error throw.
 */
export async function testRemoteEndpoint(args: {
  base_url: string;
  api_key: string;
}): Promise<string[]> {
  return invoke<string[]>("test_remote_endpoint", {
    baseUrl: args.base_url,
    apiKey: args.api_key,
  });
}

/**
 * 저장된 모든 원격 연결에서 모델 목록 조회.
 * 연결 실패 엔드포인트는 조용히 건너뜀 (best-effort).
 */
export async function listAllRemoteModels(): Promise<RemoteModelInfo[]> {
  return invoke<RemoteModelInfo[]>("list_all_remote_models");
}
