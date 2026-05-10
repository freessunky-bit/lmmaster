// 페르소나 시뮬 IPC — Personas-Korea 데이터셋 자동 다운로드 + 후속 (v0.8.x).
//
// v0.8.0: 데이터셋 상태 조회 + 자동 다운로드.
// v0.8.1+: 페르소나 정의·샘플링 / 설문 / 배치 실행 / 리포트.

import { Channel, invoke } from "@tauri-apps/api/core";

export interface PersonasDatasetStatus {
  installed: boolean;
  size_bytes: number;
  file_count: number;
}

export type PersonasDatasetEvent =
  | {
      kind: "status";
      status: string;
      file_index: number;
      file_total: number;
    }
  | {
      kind: "progress";
      completed_bytes: number;
      total_bytes: number;
      speed_bps: number;
    }
  | {
      kind: "completed";
      file_count: number;
      total_bytes: number;
    }
  | { kind: "failed"; message: string };

/** Personas-Korea 데이터셋이 캐시에 있는지 + 크기 정보. */
export async function getPersonasDatasetStatus(): Promise<PersonasDatasetStatus> {
  return invoke<PersonasDatasetStatus>("get_personas_dataset_status");
}

/** 데이터셋 자동 다운로드. Channel<PersonasDatasetEvent>로 진행 스트림. */
export async function downloadPersonasDataset(args: {
  onEvent: (e: PersonasDatasetEvent) => void;
}): Promise<void> {
  const channel = new Channel<PersonasDatasetEvent>();
  channel.onmessage = args.onEvent;
  return invoke<void>("download_personas_dataset", { channel });
}
