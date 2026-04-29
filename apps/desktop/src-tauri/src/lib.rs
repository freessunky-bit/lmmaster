//! LMmaster desktop entry library.
//!
//! 정책 (Phase 0 보강 리서치 §1, ADR-0001/0002/0003):
//! - `tauri::async_runtime::spawn` 사용 (tokio::spawn 금지 — Tauri 2가 자체 런타임 소유).
//! - `127.0.0.1:0` bind 후 `local_addr()`로 포트 획득 → frontend에 `gateway://ready` emit.
//! - `RunEvent::ExitRequested`에서 CancellationToken cancel — `WindowEvent::CloseRequested`만 사용 시
//!   Alt+F4 / taskkill / OS shutdown에서 누락(tauri#10555).

pub mod bench;
pub mod commands;
pub mod gateway;
pub mod install;
pub mod keys;
pub mod knowledge;
pub mod panic_hook;
pub mod pipelines;
pub mod presets;
pub mod registry_fetcher;
pub mod registry_provider;
pub mod runtimes;
pub mod telemetry;
pub mod updater;
pub mod workbench;
pub mod workspace;
pub mod workspaces;

use std::sync::Arc;

use key_manager::KeyManager;
use tauri::{Emitter, Manager};
use tracing_subscriber::EnvFilter;

use bench::registry::BenchRegistry;
use commands::{CatalogState, LastScanCache};
use install::registry::InstallRegistry;
use knowledge::{EmbeddingState, KnowledgeRegistry};
use presets::commands::PresetCache;
use updater::{PollerState, UpdaterRegistry};
use workbench::WorkbenchRegistry;
use workspace::commands::WorkspaceRoot;
use workspace::PortableRegistry;
use workspaces::WorkspacesState;

pub fn run() {
    // Phase 8'.0.b — panic hook 설치를 가장 먼저. tracing 등록 전에는 stderr fallback,
    // 등록 후에는 tracing 기록 + crash report 파일 + (Tauri up이면) dialog.
    // crash 디렉터리는 AppHandle이 없는 시점이라 default(`%LOCALAPPDATA%/lmmaster/crash`)로 추정.
    let crash_dir = preinit_crash_dir();
    panic_hook::install(crash_dir);

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,tauri=warn".into()),
        )
        .try_init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "LMmaster desktop starting"
    );

    let app = tauri::Builder::default()
        // Phase 8'.0.b — 단일 인스턴스. 두 번째 실행 시 첫 창 포커스 + 두 번째 즉시 종료.
        .plugin(tauri_plugin_single_instance::init(
            |app: &tauri::AppHandle, _args, _cwd| {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_focus();
                    let _ = window.unminimize();
                    let _ = window.show();
                }
            },
        ))
        .plugin(tauri_plugin_updater::Builder::new().build())
        // Phase 8'.b.2 — 외부 링크 오픈 (ToastUpdate "업데이트 보기" 등).
        // capability scope: capabilities/main.json `shell:allow-open` + URL allow `https://**`.
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::get_gateway_status,
            commands::detect_environment,
            commands::start_scan,
            commands::get_last_scan,
            commands::get_catalog,
            commands::get_recommendation,
            install::install_app,
            install::cancel_install,
            bench::commands::start_bench,
            bench::commands::cancel_bench,
            bench::commands::get_last_bench_report,
            keys::commands::create_api_key,
            keys::commands::list_api_keys,
            keys::commands::revoke_api_key,
            keys::commands::update_api_key_pipelines,
            workspace::commands::get_workspace_fingerprint,
            workspace::commands::check_workspace_repair,
            workspace::portable::start_workspace_export,
            workspace::portable::cancel_workspace_export,
            workspace::portable::start_workspace_import,
            workspace::portable::cancel_workspace_import,
            workspace::portable::verify_workspace_archive,
            runtimes::commands::list_runtime_statuses,
            runtimes::commands::list_runtime_models,
            presets::commands::get_presets,
            presets::commands::get_preset,
            workbench::start_workbench_run,
            workbench::cancel_workbench_run,
            workbench::list_workbench_runs,
            workbench::workbench_preview_jsonl,
            workbench::workbench_serialize_examples,
            workbench::list_custom_models,
            workbench::get_artifact_stats,
            workbench::cleanup_artifacts_now,
            workbench::workbench_real_status,
            workbench::lora_bootstrap_venv,
            workbench::cancel_lora_bootstrap,
            knowledge::ingest_path,
            knowledge::cancel_ingest,
            knowledge::search_knowledge,
            knowledge::list_ingests,
            knowledge::knowledge_workspace_stats,
            knowledge::list_embedding_models,
            knowledge::set_active_embedding_model,
            knowledge::download_embedding_model,
            knowledge::cancel_embedding_download,
            updater::check_for_update,
            updater::cancel_update_check,
            updater::start_auto_update_poller,
            updater::stop_auto_update_poller,
            updater::get_auto_update_status,
            pipelines::list_pipelines,
            pipelines::set_pipeline_enabled,
            pipelines::get_pipelines_config,
            pipelines::get_audit_log,
            pipelines::clear_audit_log,
            telemetry::state::get_telemetry_config,
            telemetry::state::set_telemetry_enabled,
            telemetry::state::submit_telemetry_event,
            registry_fetcher::refresh_catalog_now,
            registry_fetcher::get_last_catalog_refresh,
            workspaces::list_workspaces,
            workspaces::get_active_workspace,
            workspaces::create_workspace,
            workspaces::rename_workspace,
            workspaces::delete_workspace,
            workspaces::set_active_workspace,
        ])
        .setup(|app| {
            // 1. Gateway supervisor.
            let handle = gateway::GatewayHandle::new();
            app.manage(handle.clone());

            // 2. Install registry — Arc로 manage하면 command 안에서 clone 후 Drop guard 캡처 가능.
            let registry: Arc<InstallRegistry> = Arc::new(InstallRegistry::new());
            app.manage(registry);

            // 3. Self-scanner — broadcast 결과를 scan:summary event로 forward + 캐시.
            let last_scan: Arc<LastScanCache> = Arc::new(LastScanCache::default());
            app.manage(last_scan.clone());

            // 4. Catalog — bundled snapshot에서 모델 매니페스트 로드.
            //    빌드 시 manifests/snapshot/models/ → resource 디렉터리로 복사된 위치를 읽음.
            //    Phase 1' integration: `CatalogState`로 wrap — registry_fetcher가 갱신 시 hot-swap.
            let catalog: Arc<model_registry::Catalog> = match load_bundled_catalog(app.handle()) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    tracing::warn!(error = %e, "카탈로그 로드 실패 — 빈 카탈로그로 시작");
                    Arc::new(model_registry::Catalog::default())
                }
            };
            tracing::info!(entries = catalog.entries().len(), "카탈로그 로드 완료");
            let catalog_state: Arc<CatalogState> = Arc::new(CatalogState::new(catalog));
            app.manage(Arc::clone(&catalog_state));

            // 5. BenchRegistry — Phase 2'.c.2.
            let bench_registry: Arc<BenchRegistry> = Arc::new(BenchRegistry::new());
            app.manage(bench_registry);

            // 6. KeyManager — Phase 3'.b. SQLite at app_data_dir/keys.db.
            //    Phase 8'.0.a (ADR-0035): SQLCipher 암호화 + OS 키체인 secret + 평문 마이그레이션.
            let keys_path = app
                .path()
                .app_data_dir()
                .map(|d| d.join("keys.db"))
                .unwrap_or_else(|_| std::path::PathBuf::from("keys.db"));
            let legacy_path = app
                .path()
                .app_data_dir()
                .map(|d| d.join("keys.db.legacy"))
                .unwrap_or_else(|_| std::path::PathBuf::from("keys.db.legacy"));
            // 기존 v1 사용자 호환: 이전 평문 DB 경로(keys.db) 그대로면 마이그레이션 후 .legacy.bak로
            // rename. 새 사용자라면 keys.db는 처음부터 암호화로 생성.
            let outcome = keys::provision(&keys_path, &legacy_path);
            let key_manager: Arc<KeyManager> = match outcome.mode {
                keys::KeyStoreMode::Encrypted { passphrase } => {
                    match KeyManager::open(&keys_path, &passphrase) {
                        Ok(km) => {
                            if outcome.migrated_legacy {
                                tracing::info!("키 저장소 암호화 마이그레이션을 마쳤어요");
                            }
                            Arc::new(km)
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "암호화 KeyManager open 실패 — 메모리 폴백");
                            Arc::new(
                                KeyManager::open_memory()
                                    .expect("memory KeyManager always opens"),
                            )
                        }
                    }
                }
                keys::KeyStoreMode::UnencryptedFallback { reason } => {
                    tracing::warn!(reason, "키체인 미접근 — 평문 DB로 폴백 (Linux headless 등)");
                    match KeyManager::open_unencrypted(&keys_path) {
                        Ok(km) => Arc::new(km),
                        Err(e) => {
                            tracing::error!(error = %e, "평문 KeyManager open도 실패 — 메모리 폴백");
                            Arc::new(
                                KeyManager::open_memory()
                                    .expect("memory KeyManager always opens"),
                            )
                        }
                    }
                }
            };
            app.manage(key_manager.clone());

            // 7. WorkspaceRoot — Phase 3'.c. lazy 초기화 (첫 fingerprint 호출 시 디렉터리 생성).
            let workspace_root: Arc<WorkspaceRoot> = Arc::new(WorkspaceRoot::default());
            app.manage(workspace_root);

            // 7.b. PortableRegistry — Phase 11'. export/import 동시 다중 작업 cancel 토큰.
            let portable_registry: Arc<PortableRegistry> =
                Arc::new(PortableRegistry::new());
            app.manage(Arc::clone(&portable_registry));

            // 8. PresetCache — Phase 4.h. lazy 로드 (첫 get_presets 호출 시 manifest 적재).
            let preset_cache: Arc<PresetCache> = Arc::new(PresetCache::default());
            app.manage(preset_cache);

            // 9. WorkbenchRegistry — Phase 5'.b. 동시 다중 run 허용 (run_id uuid 키).
            let workbench_registry: Arc<WorkbenchRegistry> = Arc::new(WorkbenchRegistry::new());
            app.manage(workbench_registry);

            // 9.a. LoraBootstrapRegistry — Phase 9'.b. venv 부트스트랩 cancel 토큰.
            let lora_bootstrap_registry: Arc<workbench::LoraBootstrapRegistry> =
                Arc::new(workbench::LoraBootstrapRegistry::new());
            app.manage(lora_bootstrap_registry);

            // 9.b. ModelRegistry (custom-models) — Phase 5'.d. Workbench output 영속화.
            //      app_data_dir/registry/custom-models.json — 실패 시 in-memory 폴백.
            let custom_model_registry: Arc<model_registry::ModelRegistry> = match app
                .path()
                .app_data_dir()
            {
                Ok(d) => Arc::new(model_registry::ModelRegistry::with_dir(d.join("registry"))),
                Err(e) => {
                    tracing::warn!(error = %e, "app_data_dir 못 찾음 — custom-models를 메모리에만 보관");
                    Arc::new(model_registry::ModelRegistry::in_memory())
                }
            };
            app.manage(custom_model_registry);

            // 10. KnowledgeRegistry — Phase 4.5'.b. workspace 단위 직렬화 (workspace_id 키).
            let knowledge_registry: Arc<KnowledgeRegistry> = Arc::new(KnowledgeRegistry::new());
            app.manage(knowledge_registry);

            // 10.a. EmbeddingState — Phase 9'.a. 사용자 향 임베딩 모델 카탈로그 + 활성 모델 영속.
            //       app_data_dir/embed/active.json에 active kind를 영속, models/ 하위에 ONNX 파일 저장.
            let embedding_state: Arc<EmbeddingState> = knowledge::provision_embedding_state(app.handle());
            app.manage(Arc::clone(&embedding_state));

            // 10.b. WorkspacesState — Phase 8'.1. 사용자 향 workspace 관리 IPC.
            //       app_data_dir/workspaces/index.json — atomic rename 영속 + 첫 실행 시 default 자동 시드.
            //       active workspace 전환 시 `workspaces://changed` 이벤트 emit.
            let workspaces_state: Arc<WorkspacesState> = workspaces::provision_state(app.handle());
            app.manage(Arc::clone(&workspaces_state));

            // 11. UpdaterRegistry + PollerState — Phase 6'.b.
            //     UpdaterRegistry: 단발 check 다중 허용 (check_id uuid 키).
            //     PollerState: 자동 폴러 single-slot — 동시에 1개만 실행.
            let updater_registry: Arc<UpdaterRegistry> = Arc::new(UpdaterRegistry::new());
            app.manage(updater_registry);
            let poller_state: Arc<PollerState> = Arc::new(PollerState::new());
            app.manage(poller_state);

            // 12. PipelinesState — Phase 6'.c. Settings 토글 + 감사 로그 ring buffer.
            //     app_data_dir이 있으면 config.json 영속, 없으면 메모리 전용 폴백.
            //     Phase 6'.d — `with_audit_channel`로 receiver task spawn 후 sender를 gateway에 주입.
            let pipelines_state: Arc<pipelines::PipelinesState> = Arc::new(
                pipelines::PipelinesState::new(app.path().app_data_dir().ok()),
            );
            let audit_sender = pipelines_state.with_audit_channel();
            app.manage(Arc::clone(&pipelines_state));

            // 13. TelemetryState — Phase 7'.a. 기본 비활성, 사용자 명시 opt-in.
            //     영속 위치: app_data_dir/telemetry/config.json.
            //     Phase 7'.b: panic_hook attach — opt-in 시 panic 발생하면 GlitchTip POST 시도(DSN
            //     env 미설정 시 queue retention만).
            let telemetry_state: Arc<telemetry::TelemetryState> = Arc::new(
                telemetry::TelemetryState::new(app.path().app_data_dir().ok()),
            );
            panic_hook::attach_telemetry(Arc::clone(&telemetry_state));
            app.manage(telemetry_state);

            // 13.b. RegistryFetcherService — Phase 1'. 6시간 cron + 수동 트리거.
            //       cache_db = app_data_dir/registry/fetch.db.
            //       v1: app/runtime manifest는 카탈로그 source가 아님 (모델은 manifests/snapshot/models/).
            //       Sources GitHub Releases / jsDelivr — commit-pinned. 외부 통신 0 정책 예외 (ADR-0026 §1).
            let cache_db = app
                .path()
                .app_data_dir()
                .map(|d| d.join("registry").join("fetch.db"))
                .unwrap_or_else(|_| std::path::PathBuf::from("registry-fetch.db"));
            // bundled apps 디렉터리 — manifests/snapshot/apps/.
            let bundled_apps_dir = app
                .path()
                .resource_dir()
                .ok()
                .map(|r| r.join("manifests/snapshot/apps"))
                .filter(|p| p.exists());
            // v1 manifest IDs — 자동 갱신 대상. installer + scanner가 사용.
            let manifest_ids: Vec<String> =
                ["ollama".into(), "lm-studio".into()].to_vec();
            let registry_fetcher_service: Option<Arc<registry_fetcher::RegistryFetcherService>> =
                match tauri::async_runtime::block_on(
                    registry_fetcher::RegistryFetcherService::new(
                        cache_db,
                        bundled_apps_dir,
                        manifest_ids,
                        env!("CARGO_PKG_VERSION"),
                        env!("CARGO_PKG_VERSION"),
                    ),
                ) {
                    Ok(s) => Some(Arc::new(s)),
                    Err(e) => {
                        tracing::warn!(error = %e, "RegistryFetcherService 초기화 실패 — 자동 갱신 비활성");
                        None
                    }
                };
            if let Some(svc) = registry_fetcher_service.as_ref() {
                app.manage(Arc::clone(svc));
            }

            // 14. LiveRegistryProvider — Phase 3'.c+. Ollama / LM Studio 어댑터를 wrap해
            //     gateway가 실제로 OpenAI 호환 라우팅을 수행할 수 있게 해요.
            //     defaults_with_ollama가 11434를 사용하는 것과 일관 — 외부 통신 0 정책.
            let registry_provider: Arc<registry_provider::LiveRegistryProvider> =
                Arc::new(tauri::async_runtime::block_on(
                    registry_provider::LiveRegistryProvider::from_environment(
                        registry_provider::RuntimeEndpoints::defaults(),
                    ),
                ));
            app.manage(Arc::clone(&registry_provider));

            let app_handle_for_gateway = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    gateway::run(app_handle_for_gateway, handle, audit_sender).await
                {
                    tracing::error!(error = %e, "gateway terminated with error");
                }
            });

            // RegistryFetcher cron — 6h interval. AppHandle이 capture되지 않으면 emit이 안 되니
            // app.handle().clone()으로 명시 capture.
            if let Some(svc) = registry_fetcher_service {
                let app_handle_for_fetcher = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(e) = svc
                        .start(
                            app_handle_for_fetcher,
                            registry_fetcher::DEFAULT_INTERVAL_SECS,
                        )
                        .await
                    {
                        tracing::warn!(error = %e, "registry fetcher cron 시작 실패 — 수동 갱신만 가능");
                    }
                });
            }

            // ScannerService를 별도 task에서 mount + start. 실패해도 앱은 계속 동작.
            let app_handle_for_scan = app.handle().clone();
            let last_scan_for_task = last_scan.clone();
            tauri::async_runtime::spawn(async move {
                let opts = scanner::ScannerOptions::defaults_with_ollama("http://127.0.0.1:11434");
                let svc = match scanner::ScannerService::new(opts).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(error = %e, "scanner 초기화 실패 — self-scan 비활성");
                        return;
                    }
                };

                // Scanner를 State로 등록 — start_scan command에서 사용.
                app_handle_for_scan.manage(Arc::clone(&svc.scanner));

                // broadcast subscriber — emit + 캐시.
                let mut rx = svc.scanner.subscribe();
                let app_for_emit = app_handle_for_scan.clone();
                tauri::async_runtime::spawn(async move {
                    while let Ok(summary) = rx.recv().await {
                        last_scan_for_task.set(summary.clone());
                        if let Err(e) = app_for_emit.emit("scan:summary", &summary) {
                            tracing::debug!(error = %e, "scan:summary emit failed");
                        }
                    }
                });

                // Scheduler 시작 — 6h cron + 5분 grace.
                if let Err(e) = svc.start().await {
                    tracing::warn!(error = %e, "scanner scheduler 시작 실패");
                }

                // svc는 task가 살아있는 동안만 유지. 실용적으로는 충분 — 실제 graceful shutdown은
                // app.run RunEvent::ExitRequested에서 별도 처리 (현재는 OS shutdown에 의존).
                std::mem::forget(svc);
            });

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build LMmaster Tauri app");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            // Gateway cancel.
            if let Some(handle) = app_handle.try_state::<gateway::GatewayHandle>() {
                tracing::info!("ExitRequested received, cancelling gateway");
                handle.cancel();
            }
            // In-flight install cancel (CancellationToken::Drop은 cancel 안 함 — 명시 호출 필수).
            if let Some(registry) = app_handle.try_state::<Arc<InstallRegistry>>() {
                tracing::info!("ExitRequested received, cancelling all in-flight installs");
                registry.cancel_all();
            }
            // In-flight bench cancel.
            if let Some(registry) = app_handle.try_state::<Arc<BenchRegistry>>() {
                tracing::info!("ExitRequested received, cancelling all in-flight benches");
                registry.cancel_all();
            }
            // In-flight workbench cancel — Phase 5'.b.
            // sync 컨텍스트 → try_lock 기반 best-effort.
            if let Some(registry) = app_handle.try_state::<Arc<WorkbenchRegistry>>() {
                tracing::info!("ExitRequested received, cancelling all in-flight workbench runs");
                registry.cancel_all_blocking();
            }
            // In-flight knowledge ingest cancel — Phase 4.5'.b.
            if let Some(registry) = app_handle.try_state::<Arc<KnowledgeRegistry>>() {
                tracing::info!("ExitRequested received, cancelling all in-flight ingests");
                registry.cancel_all_blocking();
            }
            // In-flight embedding download cancel — Phase 9'.a.
            if let Some(state) = app_handle.try_state::<Arc<EmbeddingState>>() {
                tracing::info!(
                    "ExitRequested received, cancelling all in-flight embedding downloads"
                );
                state.cancel_all_blocking();
            }
            // In-flight portable export/import cancel — Phase 11'.
            if let Some(registry) = app_handle.try_state::<Arc<PortableRegistry>>() {
                tracing::info!("ExitRequested received, cancelling all in-flight portable jobs");
                registry.cancel_all_blocking();
            }
            // In-flight update check + auto-update poller — Phase 6'.b.
            if let Some(registry) = app_handle.try_state::<Arc<UpdaterRegistry>>() {
                tracing::info!("ExitRequested received, cancelling all in-flight update checks");
                updater::cancel_all_blocking(&registry);
            }
            if let Some(state) = app_handle.try_state::<Arc<PollerState>>() {
                tracing::info!("ExitRequested received, stopping auto-update poller");
                updater::stop_poller_blocking(&state);
            }
        }
    });
}

/// AppHandle 없이 추정한 crash report 디렉터리.
///
/// `panic_hook::install()`은 Tauri Builder 호출 전에 등록되니 AppHandle을 사용할 수 없어요.
/// Phase 8'.0.b: OS dirs crate 추가 의존성 없이, env / 표준 위치만 활용.
/// - Windows: `%LOCALAPPDATA%/lmmaster/crash`
/// - macOS: `~/Library/Application Support/lmmaster/crash`
/// - Linux: `$XDG_DATA_HOME/lmmaster/crash` 또는 `~/.local/share/lmmaster/crash`
fn preinit_crash_dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(local) = std::env::var("LOCALAPPDATA") {
            return Some(
                std::path::PathBuf::from(local)
                    .join("lmmaster")
                    .join("crash"),
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return Some(
                std::path::PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("lmmaster")
                    .join("crash"),
            );
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return Some(std::path::PathBuf::from(xdg).join("lmmaster").join("crash"));
        }
        if let Ok(home) = std::env::var("HOME") {
            return Some(
                std::path::PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join("lmmaster")
                    .join("crash"),
            );
        }
    }
    None
}

/// Bundled snapshot의 카탈로그 디렉터리를 찾아 로드한다.
///
/// dev 모드: 워크스페이스 root에서 `manifests/snapshot/models/` 직접 사용.
/// 프로덕션 빌드: `app.path().resource_dir()` 안의 `manifests/snapshot/models/`.
pub(crate) fn load_bundled_catalog(
    app: &tauri::AppHandle,
) -> Result<model_registry::Catalog, anyhow::Error> {
    use tauri::Manager;

    let resource_dir = app.path().resource_dir()?;
    let bundled_models = resource_dir.join("manifests/snapshot/models");
    if bundled_models.exists() {
        return Ok(model_registry::Catalog::load_from_dir(&bundled_models)?);
    }

    // dev fallback — 워크스페이스 root.
    let cwd = std::env::current_dir()?;
    for ancestor in cwd.ancestors() {
        let candidate = ancestor.join("manifests/snapshot/models");
        if candidate.exists() {
            return Ok(model_registry::Catalog::load_from_dir(&candidate)?);
        }
    }

    Err(anyhow::anyhow!(
        "manifests/snapshot/models/ 디렉터리를 찾을 수 없어요"
    ))
}
