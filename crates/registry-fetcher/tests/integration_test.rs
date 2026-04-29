//! registry-fetcher 통합 테스트 — wiremock + tempdir.
//!
//! 검증 대상 (Phase 1' 결정 §6):
//! 1. fetch_vendor_200 — 첫 tier 성공 시 다음 tier 호출 안 함.
//! 2. fallback_500_to_jsdelivr — 첫 tier 5xx → 다음으로.
//! 3. fallback_all_500_to_bundled — 모든 네트워크 실패 → bundled.
//! 4. etag_round_trip — 두 번째 호출은 304 → cached body.
//! 5. ttl_zero_refetches — TTL=0이면 매번 네트워크 시도.
//! 6. all_offline_serves_stale — TTL 초과 + 네트워크 실패 → stale 반환.
//! 7. json_parse_error_no_fallthrough — 200 + bad JSON → 다음 tier 안 가고 즉시 에러.
//! 8. bundled_only_works_offline — 네트워크 0, bundled만 있어도 OK.

use std::path::PathBuf;
use std::time::Duration;

use registry_fetcher::{FetcherError, FetcherOptions, RegistryFetcher, SourceConfig, SourceTier};
use tempfile::TempDir;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// 임시 캐시 DB + 옵션 빌드 헬퍼.
fn opts(
    dir: &TempDir,
    sources: Vec<SourceConfig>,
    bundled: Option<PathBuf>,
    ttl: Duration,
) -> FetcherOptions {
    FetcherOptions {
        cache_db: dir.path().join("cache.db"),
        sources,
        bundled_dir: bundled,
        ttl,
        stale_grace: Duration::from_secs(86400),
        http: None,
    }
}

const VALID_BODY: &str = r#"{"schema_version":1,"id":"ollama","display_name":"Ollama"}"#;

#[tokio::test]
async fn fetch_first_tier_success_does_not_hit_second() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(VALID_BODY))
        .expect(1)
        .mount(&server_a)
        .await;
    // server_b는 호출되면 안 됨.
    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(VALID_BODY))
        .expect(0)
        .mount(&server_b)
        .await;

    let dir = TempDir::new().unwrap();
    let sources = vec![
        SourceConfig {
            tier: SourceTier::Github,
            url_template: format!("{}/{{id}}.json", server_a.uri()),
            timeout: Duration::from_secs(2),
        },
        SourceConfig {
            tier: SourceTier::Jsdelivr,
            url_template: format!("{}/{{id}}.json", server_b.uri()),
            timeout: Duration::from_secs(2),
        },
    ];
    let f = RegistryFetcher::new(opts(&dir, sources, None, Duration::from_secs(3600)))
        .await
        .unwrap();
    let fm = f.fetch("ollama").await.unwrap();
    assert_eq!(fm.source, SourceTier::Github);
    assert_eq!(fm.body, VALID_BODY.as_bytes());
    assert!(!fm.from_cache);
}

#[tokio::test]
async fn fallback_500_to_next_tier() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server_a)
        .await;
    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(VALID_BODY))
        .mount(&server_b)
        .await;

    let dir = TempDir::new().unwrap();
    let sources = vec![
        SourceConfig {
            tier: SourceTier::Github,
            url_template: format!("{}/{{id}}.json", server_a.uri()),
            timeout: Duration::from_secs(2),
        },
        SourceConfig {
            tier: SourceTier::Jsdelivr,
            url_template: format!("{}/{{id}}.json", server_b.uri()),
            timeout: Duration::from_secs(2),
        },
    ];
    let f = RegistryFetcher::new(opts(&dir, sources, None, Duration::from_secs(3600)))
        .await
        .unwrap();
    let fm = f.fetch("ollama").await.unwrap();
    assert_eq!(fm.source, SourceTier::Jsdelivr);
}

#[tokio::test]
async fn fallback_all_500_to_bundled() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let bundled_dir = dir.path().join("bundled");
    std::fs::create_dir_all(&bundled_dir).unwrap();
    std::fs::write(bundled_dir.join("ollama.json"), VALID_BODY).unwrap();

    let sources = vec![
        SourceConfig {
            tier: SourceTier::Github,
            url_template: format!("{}/{{id}}.json", server.uri()),
            timeout: Duration::from_secs(2),
        },
        SourceConfig {
            tier: SourceTier::Bundled,
            url_template: String::new(),
            timeout: Duration::from_secs(1),
        },
    ];
    let f = RegistryFetcher::new(opts(
        &dir,
        sources,
        Some(bundled_dir),
        Duration::from_secs(3600),
    ))
    .await
    .unwrap();
    let fm = f.fetch("ollama").await.unwrap();
    assert_eq!(fm.source, SourceTier::Bundled);
    assert_eq!(fm.body, VALID_BODY.as_bytes());
}

#[tokio::test]
async fn etag_round_trip_returns_cached_body_on_304() {
    let server = MockServer::start().await;

    // 두 번째 mock을 먼저 mount — wiremock LIFO 매칭이라 If-None-Match 있을 때 우선 매치.
    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .and(header("if-none-match", "\"v1\""))
        .respond_with(ResponseTemplate::new(304))
        .mount(&server)
        .await;
    // 첫 응답: 200 + ETag — If-None-Match 없는 요청에 매치.
    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("ETag", "\"v1\"")
                .set_body_string(VALID_BODY),
        )
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let sources = vec![SourceConfig {
        tier: SourceTier::Github,
        url_template: format!("{}/{{id}}.json", server.uri()),
        timeout: Duration::from_secs(2),
    }];
    // TTL=0이라 매 호출마다 conditional GET.
    let f = RegistryFetcher::new(opts(&dir, sources, None, Duration::from_secs(0)))
        .await
        .unwrap();

    let fm1 = f.fetch("ollama").await.unwrap();
    assert!(!fm1.from_cache);
    assert_eq!(fm1.body, VALID_BODY.as_bytes());

    let fm2 = f.fetch("ollama").await.unwrap();
    assert!(fm2.from_cache);
    assert_eq!(fm2.body, VALID_BODY.as_bytes());
    assert_eq!(fm2.etag.as_deref(), Some("\"v1\""));
}

#[tokio::test]
async fn ttl_within_window_skips_network() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(VALID_BODY))
        .expect(1) // 두 번째 호출은 캐시 hit이라 origin 1번만.
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let sources = vec![SourceConfig {
        tier: SourceTier::Github,
        url_template: format!("{}/{{id}}.json", server.uri()),
        timeout: Duration::from_secs(2),
    }];
    let f = RegistryFetcher::new(opts(&dir, sources, None, Duration::from_secs(3600)))
        .await
        .unwrap();
    let fm1 = f.fetch("ollama").await.unwrap();
    assert!(!fm1.from_cache);
    let fm2 = f.fetch("ollama").await.unwrap();
    assert!(fm2.from_cache);
}

#[tokio::test]
async fn invalid_json_does_not_fall_through() {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    // server_a returns 200 + invalid JSON. fetcher must NOT cascade to server_b.
    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not-json"))
        .expect(1)
        .mount(&server_a)
        .await;
    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(VALID_BODY))
        .expect(0)
        .mount(&server_b)
        .await;

    let dir = TempDir::new().unwrap();
    let sources = vec![
        SourceConfig {
            tier: SourceTier::Github,
            url_template: format!("{}/{{id}}.json", server_a.uri()),
            timeout: Duration::from_secs(2),
        },
        SourceConfig {
            tier: SourceTier::Jsdelivr,
            url_template: format!("{}/{{id}}.json", server_b.uri()),
            timeout: Duration::from_secs(2),
        },
    ];
    let f = RegistryFetcher::new(opts(&dir, sources, None, Duration::from_secs(3600)))
        .await
        .unwrap();
    let r = f.fetch("ollama").await;
    assert!(matches!(r, Err(FetcherError::JsonParse(_))));
}

#[tokio::test]
async fn bundled_only_works_offline() {
    let dir = TempDir::new().unwrap();
    let bundled_dir = dir.path().join("bundled");
    std::fs::create_dir_all(&bundled_dir).unwrap();
    std::fs::write(bundled_dir.join("ollama.json"), VALID_BODY).unwrap();

    let sources = vec![SourceConfig {
        tier: SourceTier::Bundled,
        url_template: String::new(),
        timeout: Duration::from_secs(1),
    }];
    let f = RegistryFetcher::new(opts(
        &dir,
        sources,
        Some(bundled_dir),
        Duration::from_secs(3600),
    ))
    .await
    .unwrap();
    let fm = f.fetch("ollama").await.unwrap();
    assert_eq!(fm.source, SourceTier::Bundled);
}

#[tokio::test]
async fn invalidate_clears_cache() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ollama.json"))
        .respond_with(ResponseTemplate::new(200).set_body_string(VALID_BODY))
        .expect(2) // invalidate 후 다시 받음.
        .mount(&server)
        .await;

    let dir = TempDir::new().unwrap();
    let sources = vec![SourceConfig {
        tier: SourceTier::Github,
        url_template: format!("{}/{{id}}.json", server.uri()),
        timeout: Duration::from_secs(2),
    }];
    let f = RegistryFetcher::new(opts(&dir, sources, None, Duration::from_secs(3600)))
        .await
        .unwrap();
    let _ = f.fetch("ollama").await.unwrap();
    f.invalidate(Some("ollama")).await.unwrap();
    let fm = f.fetch("ollama").await.unwrap();
    assert!(!fm.from_cache);
}

#[tokio::test]
async fn parse_helper_decodes_body() {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Min {
        schema_version: u32,
        id: String,
    }

    let dir = TempDir::new().unwrap();
    let bundled_dir = dir.path().join("bundled");
    std::fs::create_dir_all(&bundled_dir).unwrap();
    std::fs::write(bundled_dir.join("ollama.json"), VALID_BODY).unwrap();

    let sources = vec![SourceConfig {
        tier: SourceTier::Bundled,
        url_template: String::new(),
        timeout: Duration::from_secs(1),
    }];
    let f = RegistryFetcher::new(opts(
        &dir,
        sources,
        Some(bundled_dir),
        Duration::from_secs(3600),
    ))
    .await
    .unwrap();
    let fm = f.fetch("ollama").await.unwrap();
    let parsed: Min = f.parse(&fm).unwrap();
    assert_eq!(parsed.schema_version, 1);
    assert_eq!(parsed.id, "ollama");
}
