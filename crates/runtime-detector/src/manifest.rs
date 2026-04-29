//! Pinokio-style declarative app manifest evaluator.
//!
//! 정책 (ADR-0017, Phase 1A.2.b 보강 리서치):
//! - Manifest의 `detect` 배열을 순서대로 실행 → 첫 "running" 매치는 즉시 반환.
//! - 다른 룰은 platform 필터로 자동 skip (manifest는 portable, runtime이 분기).
//! - `registry.read`는 cfg(windows)에서만 실제 동작 — 다른 OS에선 자동 Skipped.
//! - 모든 IO 실패는 warn 로그 후 NoMatch로 격하 — 평가는 절대 panic 안 함.
//! - 결과 aggregation: running > installed > not_installed.
//!
//! Phase 1A.2.b 책임 영역:
//! - AppManifest 파싱 (manifests/apps/*.json)
//! - DetectRule 4종(http.get / shell.which / registry.read / fs.exists) 평가
//! - EvalContext에 외부 reqwest::Client 주입 — Detector가 보유한 단일 connection pool 재사용
//!
//! Phase 1A.3 합류 예정:
//! - install/update 액션 평가는 별도 trait/enum (`Action`)으로 분리 예정.
//! - shell.which의 `version_command` 실제 spawn은 1A.3 (capability ACL 필요).
//! - fs.exists의 plist 필드 추출은 1A.3 (mac).

use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::Status;

/// `manifests/apps/<id>.json`의 deserialize 대상. 알지 못하는 필드는 silently 무시한다.
#[derive(Debug, Clone, Deserialize)]
pub struct AppManifest {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub license: String,
    #[serde(default)]
    pub redistribution_allowed: bool,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    pub detect: Vec<DetectRule>,
    /// Phase 1A.3 합류: 설치 스펙 (platform별 분기). installer crate가 deserialize 후 실행.
    #[serde(default)]
    pub install: Option<InstallSpec>,
    /// 자동 갱신 정책 (ADR-0019, Phase 6'). source(github_release / vendor_endpoint) + trigger.
    #[serde(default)]
    pub update: Option<UpdateSpec>,
}

/// 플랫폼별 install action.
#[derive(Debug, Clone, Deserialize)]
pub struct InstallSpec {
    #[serde(default)]
    pub windows: Option<PlatformInstall>,
    #[serde(default)]
    pub macos: Option<PlatformInstall>,
    #[serde(default)]
    pub linux: Option<PlatformInstall>,
}

impl InstallSpec {
    /// 현재 platform에 해당하는 install action을 반환. 없으면 None.
    pub fn for_current_platform(&self) -> Option<&PlatformInstall> {
        match Platform::current() {
            Platform::Windows => self.windows.as_ref(),
            Platform::Macos => self.macos.as_ref(),
            Platform::Linux => self.linux.as_ref(),
        }
    }
}

/// 4 method 분기. `serde(tag = "method", rename_all = "snake_case")`.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum PlatformInstall {
    /// installer 다운로드 후 실행 (Inno Setup, NSIS, MSI 등).
    DownloadAndRun(DownloadAndRunSpec),
    /// archive 다운로드 후 압축 해제 (Phase 1A.3.b.2). 본 sub-phase에서는 schema만.
    DownloadAndExtract(DownloadAndExtractSpec),
    /// `curl ... | sh` 실행 (Linux). 본 sub-phase에서는 schema만.
    #[serde(rename = "shell.curl_pipe_sh")]
    ShellCurlPipeSh(ShellCurlPipeShSpec),
    /// 외부 URL을 사용자 default 브라우저로 연다 (LM Studio EULA 대응).
    OpenUrl(OpenUrlSpec),
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadAndRunSpec {
    pub url_template: String,
    #[serde(default)]
    pub version_url: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub min_disk_mb: Option<u64>,
    #[serde(default)]
    pub min_ram_mb: Option<u64>,
    /// 추가 성공 코드(0 외에). 예: MSI 3010(reboot required), 1641(reboot initiated).
    #[serde(default)]
    pub success_exit_codes: Vec<i32>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub post_install_check: Option<PostInstallCheck>,
    /// SHA256 (32-byte hex). manifest에 sha256 필드가 있으면 검증 — 없으면 skip(권장 X).
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DownloadAndExtractSpec {
    pub url_template: String,
    #[serde(default)]
    pub version_url: Option<String>,
    pub extract_to: String,
    #[serde(default)]
    pub min_disk_mb: Option<u64>,
    #[serde(default)]
    pub min_ram_mb: Option<u64>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub post_install_check: Option<PostInstallCheck>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellCurlPipeShSpec {
    pub url_template: String,
    #[serde(default)]
    pub min_disk_mb: Option<u64>,
    #[serde(default)]
    pub min_ram_mb: Option<u64>,
    #[serde(default)]
    pub warning_ko: Option<String>,
    #[serde(default)]
    pub post_install_check: Option<PostInstallCheck>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenUrlSpec {
    pub url: String,
    #[serde(default)]
    pub reason_ko: Option<String>,
}

/// 설치 후 검증. method = "http.get"만 v1 지원. 결과 fail이어도 install action은 success로 본다
/// (네트워크 일시 장애 등) — 호출자가 표시 결정.
#[derive(Debug, Clone, Deserialize)]
pub struct PostInstallCheck {
    pub method: String,
    pub url: String,
    #[serde(default = "default_wait_seconds")]
    pub wait_seconds: u32,
}

fn default_wait_seconds() -> u32 {
    30
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateSpec {
    pub source: UpdateSource,
    pub trigger: UpdateTrigger,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UpdateSource {
    GithubRelease { repo: String },
    VendorEndpoint { url: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method", rename_all = "snake_case")]
pub enum UpdateTrigger {
    /// 같은 install spec을 다시 실행.
    RerunInstall,
    /// 사용자에게 URL을 연다 (vendor가 자체 updater 보유).
    OpenUrl { url: String },
}

/// detect rule — `serde(tag = "method", rename_all = "kebab-case")`로 method 문자열 dispatch.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method")]
pub enum DetectRule {
    #[serde(rename = "http.get")]
    HttpGet(HttpGetRule),
    #[serde(rename = "shell.which")]
    ShellWhich(ShellWhichRule),
    #[serde(rename = "registry.read")]
    RegistryRead(RegistryReadRule),
    #[serde(rename = "fs.exists")]
    FsExists(FsExistsRule),
}

#[derive(Debug, Clone, Deserialize)]
pub struct HttpGetRule {
    pub url: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub expect_status: Option<u16>,
    #[serde(default)]
    pub extract_field: Option<String>,
    #[serde(default)]
    pub indicates: Option<Indicates>,
    #[serde(default)]
    pub platform: Option<Platform>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShellWhichRule {
    pub cmd: String,
    /// 실제 spawn은 Phase 1A.3에서 (현재 evaluator는 path 존재 여부만 확인).
    #[serde(default)]
    pub version_command: Option<Vec<String>>,
    #[serde(default)]
    pub indicates: Option<Indicates>,
    #[serde(default)]
    pub platform: Option<Platform>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistryReadRule {
    pub hive: RegistryHive,
    pub subkey_glob: String,
    pub match_value: MatchValue,
    #[serde(default)]
    pub extract_value: Option<String>,
    #[serde(default)]
    pub indicates: Option<Indicates>,
    #[serde(default)]
    pub platform: Option<Platform>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MatchValue {
    pub key: String,
    pub regex: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FsExistsRule {
    pub path: String,
    /// mac 전용 — Info.plist에서 추출할 필드. Phase 1A.3에서 실제 파싱.
    #[serde(default)]
    pub extract_plist_field: Option<String>,
    #[serde(default)]
    pub indicates: Option<Indicates>,
    #[serde(default)]
    pub platform: Option<Platform>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Indicates {
    Running,
    Installed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    Windows,
    Macos,
    Linux,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RegistryHive {
    Hklm,
    Hkcu,
}

/// Evaluator 호출 시 주입되는 컨텍스트. http는 Detector가 이미 보유한 client를 재사용.
pub struct EvalContext<'a> {
    pub http: &'a reqwest::Client,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuleOutcome {
    Running {
        version: Option<String>,
        endpoint: Option<String>,
    },
    Installed {
        version: Option<String>,
    },
    NoMatch,
    Skipped {
        reason: String,
    },
    Err {
        message: String,
    },
}

impl RuleOutcome {
    fn label(&self) -> &'static str {
        match self {
            Self::Running { .. } => "running",
            Self::Installed { .. } => "installed",
            Self::NoMatch => "no-match",
            Self::Skipped { .. } => "skipped",
            Self::Err { .. } => "err",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleDiagnostic {
    pub method: String,
    pub outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvalResult {
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched_rule: Option<String>,
    pub diagnostics: Vec<RuleDiagnostic>,
}

pub struct ManifestEvaluator {
    pub manifest: AppManifest,
}

impl ManifestEvaluator {
    /// JSON 문자열에서 manifest 파싱.
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        let manifest: AppManifest = serde_json::from_str(s)?;
        Ok(Self { manifest })
    }

    pub fn from_path(p: &Path) -> anyhow::Result<Self> {
        let s = std::fs::read_to_string(p)?;
        let m = Self::from_json(&s)?;
        Ok(m)
    }

    /// detect 배열 순회. running 즉시 반환, 그 외엔 installed/not_installed로 aggregate.
    pub async fn evaluate(&self, ctx: &EvalContext<'_>) -> EvalResult {
        let mut diagnostics: Vec<RuleDiagnostic> = Vec::with_capacity(self.manifest.detect.len());
        let mut installed_version: Option<String> = None;
        let mut matched_installed_rule: Option<String> = None;
        let current = Platform::current();

        for rule in &self.manifest.detect {
            let method_name = rule.method_name();

            // Platform filter — 다른 OS 룰은 silently skip.
            if let Some(p) = rule.platform_constraint() {
                if p != current {
                    diagnostics.push(RuleDiagnostic {
                        method: method_name.into(),
                        outcome: "skipped".into(),
                        detail: Some(format!("platform={:?}", p)),
                    });
                    continue;
                }
            }

            let outcome = rule.evaluate(ctx).await;
            diagnostics.push(RuleDiagnostic {
                method: method_name.into(),
                outcome: outcome.label().into(),
                detail: outcome_detail_string(&outcome),
            });

            match outcome {
                RuleOutcome::Running { version, endpoint } => {
                    return EvalResult {
                        status: Status::Running,
                        version,
                        endpoint,
                        matched_rule: Some(method_name.into()),
                        diagnostics,
                    };
                }
                RuleOutcome::Installed { version } => {
                    if installed_version.is_none() && version.is_some() {
                        installed_version = version;
                    }
                    if matched_installed_rule.is_none() {
                        matched_installed_rule = Some(method_name.into());
                    }
                }
                RuleOutcome::Err { ref message } => {
                    tracing::warn!(rule = method_name, error = %message, "detect rule errored — continuing");
                }
                RuleOutcome::NoMatch | RuleOutcome::Skipped { .. } => {}
            }
        }

        if matched_installed_rule.is_some() {
            EvalResult {
                status: Status::Installed,
                version: installed_version,
                endpoint: None,
                matched_rule: matched_installed_rule,
                diagnostics,
            }
        } else {
            EvalResult {
                status: Status::NotInstalled,
                version: None,
                endpoint: None,
                matched_rule: None,
                diagnostics,
            }
        }
    }
}

impl DetectRule {
    fn platform_constraint(&self) -> Option<Platform> {
        match self {
            Self::HttpGet(r) => r.platform,
            Self::ShellWhich(r) => r.platform,
            Self::RegistryRead(r) => r.platform,
            Self::FsExists(r) => r.platform,
        }
    }

    fn method_name(&self) -> &'static str {
        match self {
            Self::HttpGet(_) => "http.get",
            Self::ShellWhich(_) => "shell.which",
            Self::RegistryRead(_) => "registry.read",
            Self::FsExists(_) => "fs.exists",
        }
    }

    async fn evaluate(&self, ctx: &EvalContext<'_>) -> RuleOutcome {
        match self {
            Self::HttpGet(r) => evaluate_http_get(ctx, r).await,
            Self::ShellWhich(r) => evaluate_shell_which(r),
            Self::RegistryRead(r) => evaluate_registry_read(r),
            Self::FsExists(r) => evaluate_fs_exists(r),
        }
    }
}

async fn evaluate_http_get(ctx: &EvalContext<'_>, rule: &HttpGetRule) -> RuleOutcome {
    let mut req = ctx.http.get(&rule.url);
    if let Some(ms) = rule.timeout_ms {
        req = req.timeout(Duration::from_millis(ms));
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) if e.is_connect() || e.is_timeout() => return RuleOutcome::NoMatch,
        Err(e) => {
            return RuleOutcome::Err {
                message: e.to_string(),
            }
        }
    };

    let expected = rule.expect_status.unwrap_or(200);
    if resp.status().as_u16() != expected {
        return RuleOutcome::NoMatch;
    }

    let version = if let Some(field) = &rule.extract_field {
        match resp.json::<serde_json::Value>().await {
            Ok(v) => v.get(field).and_then(|x| x.as_str()).map(String::from),
            Err(_) => None,
        }
    } else {
        None
    };

    let endpoint = url_to_origin(&rule.url);
    match rule.indicates {
        Some(Indicates::Running) | None => RuleOutcome::Running {
            version,
            endpoint: Some(endpoint),
        },
        Some(Indicates::Installed) => RuleOutcome::Installed { version },
    }
}

/// 매우 단순한 URL origin 추출 — `scheme://host[:port]`만. 추가 의존성 회피.
fn url_to_origin(url: &str) -> String {
    if let Some(scheme_end) = url.find("://") {
        let after = &url[scheme_end + 3..];
        let host_end = after.find('/').unwrap_or(after.len());
        format!("{}://{}", &url[..scheme_end], &after[..host_end])
    } else {
        url.to_string()
    }
}

fn evaluate_shell_which(rule: &ShellWhichRule) -> RuleOutcome {
    match which::which(&rule.cmd) {
        Ok(path) => {
            tracing::debug!(cmd = %rule.cmd, path = %path.display(), "shell.which found");
            match rule.indicates {
                Some(Indicates::Running) => RuleOutcome::Running {
                    version: None,
                    endpoint: None,
                },
                Some(Indicates::Installed) | None => RuleOutcome::Installed { version: None },
            }
        }
        Err(_) => RuleOutcome::NoMatch,
    }
}

#[cfg(windows)]
fn evaluate_registry_read(rule: &RegistryReadRule) -> RuleOutcome {
    use winreg::enums::*;
    use winreg::RegKey;

    let hive_handle = match rule.hive {
        RegistryHive::Hklm => HKEY_LOCAL_MACHINE,
        RegistryHive::Hkcu => HKEY_CURRENT_USER,
    };
    let root = RegKey::predef(hive_handle);

    // `subkey_glob`의 trailing `\*` 또는 `*`는 enumerate 의도 표시 — winreg가 자동 enumerate하므로 strip.
    let subkey = rule
        .subkey_glob
        .trim_end_matches('*')
        .trim_end_matches('\\');

    let parent = match root.open_subkey_with_flags(subkey, KEY_READ | KEY_WOW64_64KEY) {
        Ok(k) => k,
        Err(_) => return RuleOutcome::NoMatch,
    };

    let regex = match regex::Regex::new(&rule.match_value.regex) {
        Ok(r) => r,
        Err(e) => {
            return RuleOutcome::Err {
                message: format!("invalid regex {:?}: {}", rule.match_value.regex, e),
            }
        }
    };

    for sub in parent.enum_keys().flatten() {
        let Ok(child) = parent.open_subkey(&sub) else {
            continue;
        };
        let Ok(value) = child.get_value::<String, _>(&rule.match_value.key) else {
            continue;
        };
        if regex.is_match(&value) {
            let version = rule
                .extract_value
                .as_ref()
                .and_then(|name| child.get_value::<String, _>(name).ok());
            return match rule.indicates {
                Some(Indicates::Running) => RuleOutcome::Running {
                    version,
                    endpoint: None,
                },
                Some(Indicates::Installed) | None => RuleOutcome::Installed { version },
            };
        }
    }
    RuleOutcome::NoMatch
}

#[cfg(not(windows))]
fn evaluate_registry_read(_rule: &RegistryReadRule) -> RuleOutcome {
    RuleOutcome::Skipped {
        reason: "registry.read는 Windows 전용".into(),
    }
}

fn evaluate_fs_exists(rule: &FsExistsRule) -> RuleOutcome {
    if std::path::Path::new(&rule.path).exists() {
        match rule.indicates {
            Some(Indicates::Running) => RuleOutcome::Running {
                version: None,
                endpoint: None,
            },
            Some(Indicates::Installed) | None => RuleOutcome::Installed { version: None },
        }
    } else {
        RuleOutcome::NoMatch
    }
}

fn outcome_detail_string(o: &RuleOutcome) -> Option<String> {
    match o {
        RuleOutcome::Running { version, endpoint } => Some(format!(
            "version={} endpoint={}",
            version.as_deref().unwrap_or("-"),
            endpoint.as_deref().unwrap_or("-"),
        )),
        RuleOutcome::Installed { version } => {
            Some(format!("version={}", version.as_deref().unwrap_or("-")))
        }
        RuleOutcome::Skipped { reason } => Some(reason.clone()),
        RuleOutcome::Err { message } => Some(message.clone()),
        RuleOutcome::NoMatch => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_millis(500))
            .connect_timeout(Duration::from_millis(200))
            .no_proxy()
            .build()
            .unwrap()
    }

    fn manifest_with_rules(rules: Vec<DetectRule>) -> ManifestEvaluator {
        ManifestEvaluator {
            manifest: AppManifest {
                schema_version: 1,
                id: "test".into(),
                display_name: "Test".into(),
                license: "MIT".into(),
                redistribution_allowed: true,
                homepage: None,
                category: None,
                detect: rules,
                install: None,
                update: None,
            },
        }
    }

    #[tokio::test]
    async fn http_running_short_circuits() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "0.5.7"})),
            )
            .mount(&server)
            .await;
        let url = format!("{}/api/version", server.uri());

        let evaluator = manifest_with_rules(vec![DetectRule::HttpGet(HttpGetRule {
            url: url.clone(),
            timeout_ms: Some(500),
            expect_status: Some(200),
            extract_field: Some("version".into()),
            indicates: Some(Indicates::Running),
            platform: None,
        })]);

        let client = http_client();
        let r = evaluator.evaluate(&EvalContext { http: &client }).await;
        assert_eq!(r.status, Status::Running);
        assert_eq!(r.version.as_deref(), Some("0.5.7"));
        assert_eq!(r.matched_rule.as_deref(), Some("http.get"));
    }

    #[tokio::test]
    async fn http_unreachable_aggregates_to_not_installed() {
        let evaluator = manifest_with_rules(vec![DetectRule::HttpGet(HttpGetRule {
            url: "http://127.0.0.1:1/never".into(),
            timeout_ms: Some(200),
            expect_status: Some(200),
            extract_field: None,
            indicates: Some(Indicates::Running),
            platform: None,
        })]);

        let client = http_client();
        let r = evaluator.evaluate(&EvalContext { http: &client }).await;
        assert_eq!(r.status, Status::NotInstalled);
        assert!(r.matched_rule.is_none());
    }

    #[tokio::test]
    async fn http_installed_then_no_running_returns_installed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/info"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"v": "1.2.3"})),
            )
            .mount(&server)
            .await;

        let evaluator = manifest_with_rules(vec![DetectRule::HttpGet(HttpGetRule {
            url: format!("{}/info", server.uri()),
            timeout_ms: Some(500),
            expect_status: Some(200),
            extract_field: Some("v".into()),
            indicates: Some(Indicates::Installed),
            platform: None,
        })]);

        let client = http_client();
        let r = evaluator.evaluate(&EvalContext { http: &client }).await;
        assert_eq!(r.status, Status::Installed);
        assert_eq!(r.version.as_deref(), Some("1.2.3"));
    }

    #[tokio::test]
    async fn platform_filter_skips_non_current() {
        let foreign_platform = if cfg!(windows) {
            Platform::Linux
        } else {
            Platform::Windows
        };
        let evaluator = manifest_with_rules(vec![DetectRule::FsExists(FsExistsRule {
            path: "/this/path/does/not/exist".into(),
            extract_plist_field: None,
            indicates: Some(Indicates::Installed),
            platform: Some(foreign_platform),
        })]);

        let client = http_client();
        let r = evaluator.evaluate(&EvalContext { http: &client }).await;
        assert_eq!(r.status, Status::NotInstalled);
        assert_eq!(
            r.diagnostics[0].outcome, "skipped",
            "foreign platform rule should be skipped"
        );
    }

    #[tokio::test]
    async fn fs_exists_finds_temp_dir() {
        let dir = std::env::temp_dir();
        let evaluator = manifest_with_rules(vec![DetectRule::FsExists(FsExistsRule {
            path: dir.to_string_lossy().into_owned(),
            extract_plist_field: None,
            indicates: Some(Indicates::Installed),
            platform: None,
        })]);

        let client = http_client();
        let r = evaluator.evaluate(&EvalContext { http: &client }).await;
        assert_eq!(r.status, Status::Installed);
    }

    #[tokio::test]
    async fn shell_which_skipped_when_absent() {
        let evaluator = manifest_with_rules(vec![DetectRule::ShellWhich(ShellWhichRule {
            cmd: "this-binary-definitely-does-not-exist-12345xyz".into(),
            version_command: None,
            indicates: Some(Indicates::Installed),
            platform: None,
        })]);

        let client = http_client();
        let r = evaluator.evaluate(&EvalContext { http: &client }).await;
        assert_eq!(r.status, Status::NotInstalled);
    }

    #[tokio::test]
    async fn full_aggregation_running_overrides_installed() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/version"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"version": "1.0.0"})),
            )
            .mount(&server)
            .await;

        let evaluator = manifest_with_rules(vec![
            // First rule: fs.exists — installed (always true).
            DetectRule::FsExists(FsExistsRule {
                path: std::env::temp_dir().to_string_lossy().into_owned(),
                extract_plist_field: None,
                indicates: Some(Indicates::Installed),
                platform: None,
            }),
            // Second rule: http.get — running.
            DetectRule::HttpGet(HttpGetRule {
                url: format!("{}/api/version", server.uri()),
                timeout_ms: Some(500),
                expect_status: Some(200),
                extract_field: Some("version".into()),
                indicates: Some(Indicates::Running),
                platform: None,
            }),
        ]);

        let client = http_client();
        let r = evaluator.evaluate(&EvalContext { http: &client }).await;
        // running takes precedence over earlier-matched installed.
        assert_eq!(r.status, Status::Running);
        assert_eq!(r.version.as_deref(), Some("1.0.0"));
    }

    #[test]
    fn parse_real_ollama_manifest() {
        let json = r#"{
            "schema_version": 1,
            "id": "ollama",
            "display_name": "Ollama",
            "license": "MIT",
            "redistribution_allowed": true,
            "homepage": "https://ollama.com",
            "github_repo": "ollama/ollama",
            "category": "external-runtime",
            "detect": [
                { "method": "http.get", "url": "http://127.0.0.1:11434/api/version", "timeout_ms": 1500, "expect_status": 200, "extract_field": "version", "indicates": "running" },
                { "method": "shell.which", "cmd": "ollama", "indicates": "installed" },
                { "method": "fs.exists", "platform": "macos", "path": "/Applications/Ollama.app", "indicates": "installed" }
            ]
        }"#;
        let m = ManifestEvaluator::from_json(json).expect("parse ok");
        assert_eq!(m.manifest.id, "ollama");
        assert_eq!(m.manifest.detect.len(), 3);
    }

    #[test]
    fn parse_real_lm_studio_manifest_with_registry() {
        let json = r#"{
            "schema_version": 1,
            "id": "lm-studio",
            "display_name": "LM Studio",
            "license": "Element Labs EULA",
            "redistribution_allowed": false,
            "detect": [
                { "method": "http.get", "url": "http://127.0.0.1:1234/v1/models", "timeout_ms": 1500, "expect_status": 200 },
                { "method": "registry.read", "platform": "windows", "hive": "HKCU", "subkey_glob": "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*", "match_value": { "key": "DisplayName", "regex": "^LM Studio" }, "extract_value": "DisplayVersion", "indicates": "installed" }
            ]
        }"#;
        let m = ManifestEvaluator::from_json(json).expect("parse ok");
        assert!(matches!(&m.manifest.detect[1], DetectRule::RegistryRead(_)));
    }

    #[test]
    fn url_to_origin_strips_path() {
        assert_eq!(
            url_to_origin("http://127.0.0.1:11434/api/version"),
            "http://127.0.0.1:11434"
        );
        assert_eq!(url_to_origin("https://example.com"), "https://example.com");
        assert_eq!(url_to_origin("http://example.com/"), "http://example.com");
    }

    #[tokio::test]
    async fn from_path_loads_manifests_apps_ollama_json() {
        // 실제 repo의 manifests/apps/ollama.json을 파싱 (CARGO_MANIFEST_DIR 기준).
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("manifests/apps/ollama.json");
        if !p.exists() {
            // 워크스페이스 외 빌드 시(예: cargo install)엔 파일이 없을 수 있음 — skip.
            eprintln!("skip: {} not found", p.display());
            return;
        }
        let m = ManifestEvaluator::from_path(&p).expect("parse manifests/apps/ollama.json");
        assert_eq!(m.manifest.id, "ollama");
        assert!(!m.manifest.detect.is_empty());
    }
}
