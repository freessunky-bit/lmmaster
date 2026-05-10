//! 페르소나 시뮬레이션 v0.8.x — 가상 한국인 100인 설문 시뮬.
//!
//! 정책:
//! - Python 0 — 데이터셋 다운로드부터 결과 가공까지 LMmaster 자체 처리.
//! - Personas-Korea 데이터셋(nvidia/Nemotron-Personas-Korea, CC BY 4.0)을 HF API + reqwest로 직접 다운.
//! - 캐시 위치: `app_local_data_dir()/personas/`.
//! - v0.8.0: 데이터셋 자동 다운로드 + 진행률 GUI.
//! - v0.8.1+: 페르소나 정의 / 설문 / 배치 실행 / 리포트.

pub mod dataset;
