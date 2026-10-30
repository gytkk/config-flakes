#[path = "support/fake_kis_server.rs"]
mod fake_kis_server;
#[path = "support/fixtures.rs"]
mod fixtures;

use std::sync::Arc;
use std::time::Duration;

use fake_kis_server::{FakeKisServer, FakeResponse};
use fixtures::load_fixture;
use serde_json::json;
use time::macros::datetime;
use trading_agent_kis_client::auth::{
    KisAuthConfig, KisCredential, KisTokenProvider, KisTokenResponse, TokenIssueRequest,
};
use trading_agent_kis_client::domain::{AccountKey, BrokerEnvironment};

#[test]
fn token_issue_request_uses_environment_specific_endpoint_and_json_body() {
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let config = KisAuthConfig::new(BrokerEnvironment::Paper, credential).expect("auth config");

    let request = TokenIssueRequest::build(&config).expect("token request");

    assert_eq!(request.method(), "POST");
    assert_eq!(
        request.url(),
        "https://openapivts.koreainvestment.com:29443/oauth2/tokenP"
    );
    assert_eq!(
        request.headers().get("content-type").map(String::as_str),
        Some("application/json")
    );
    assert_eq!(request.body().grant_type, "client_credentials");
    assert_eq!(request.body().app_key, "paper-app-key");
    assert_eq!(request.body().app_secret, "paper-app-secret");
}

#[test]
fn token_issue_request_debug_redacts_credentials() {
    let credential = KisCredential::new("paper-app-key", "paper-app-secret").expect("credential");
    let config = KisAuthConfig::new(BrokerEnvironment::Paper, credential).expect("auth config");
    let request = TokenIssueRequest::build(&config).expect("token request");

    let debug = format!("{request:?}");

    assert!(!debug.contains("paper-app-key"));
    assert!(!debug.contains("paper-app-secret"));
    assert!(debug.contains("***"));
}

#[test]
fn live_token_issue_request_uses_live_base_url() {
    let credential = KisCredential::new("live-app-key", "live-app-secret").expect("credential");
    let config = KisAuthConfig::new(BrokerEnvironment::Live, credential).expect("auth config");

    let request = TokenIssueRequest::build(&config).expect("token request");

    assert_eq!(
        request.url(),
        "https://openapi.koreainvestment.com:9443/oauth2/tokenP"
    );
}

#[test]
fn credential_debug_redacts_app_key_and_secret() {
    let credential = KisCredential::new("real-app-key", "real-app-secret").expect("credential");

    let debug = format!("{credential:?}");

    assert!(!debug.contains("real-app-key"));
    assert!(!debug.contains("real-app-secret"));
    assert!(debug.contains("***"));
}

#[test]
fn auth_config_debug_redacts_account_key() {
    let credential = KisCredential::new("real-app-key", "real-app-secret").expect("credential");
    let config = KisAuthConfig::new(BrokerEnvironment::Paper, credential)
        .expect("auth config")
        .with_account_key(AccountKey::new("12345678-01").expect("account key"));

    let debug = format!("{config:?}");

    assert!(!debug.contains("12345678"));
    assert!(!debug.contains("real-app-key"));
    assert!(!debug.contains("real-app-secret"));
    assert!(debug.contains("***"));
}

#[test]
fn token_response_maps_kis_expiry_string_to_utc_snapshot() {
    let raw: KisTokenResponse = serde_json::from_str(&load_fixture("auth/token_success.json"))
        .expect("valid token response");

    let snapshot = raw.to_snapshot().expect("token snapshot");

    assert_eq!(snapshot.access_token.as_str(), "fixture-token");
    assert_eq!(snapshot.token_type, "Bearer");
    assert_eq!(snapshot.expires_in_seconds, 86_400);
    assert_eq!(snapshot.expires_at_utc, datetime!(2026-04-24 00:30:00 UTC));
}

#[test]
fn token_snapshot_debug_redacts_access_token() {
    let raw: KisTokenResponse = serde_json::from_value(json!({
        "access_token": "token-value",
        "token_type": "Bearer",
        "expires_in": 86400,
        "access_token_token_expired": "2026-04-24 09:30:00"
    }))
    .expect("valid token response");

    let snapshot = raw.to_snapshot().expect("token snapshot");
    let debug = format!("{snapshot:?}");

    assert!(!debug.contains("token-value"));
    assert!(debug.contains("***"));
}

#[tokio::test]
async fn token_provider_issues_token_and_reuses_memory_cache_before_expiry() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"issued-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    let provider = token_provider(BrokerEnvironment::Paper, server.base_url(), None);

    let first = provider
        .access_token(false)
        .await
        .expect("first token issue");
    let second = provider
        .access_token(false)
        .await
        .expect("cached token access");

    assert_eq!(first.as_str(), "issued-token");
    assert_eq!(second.as_str(), "issued-token");
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn token_provider_reuses_valid_file_cache_without_network() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"persisted-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_path = tempdir.path().join("kis-token.json");
    let provider = token_provider(
        BrokerEnvironment::Paper,
        server.base_url(),
        Some(cache_path.clone()),
    );

    provider
        .access_token(false)
        .await
        .expect("first token issue writes cache");
    let second_provider = token_provider(
        BrokerEnvironment::Paper,
        server.base_url(),
        Some(cache_path),
    );
    let cached = second_provider
        .access_token(false)
        .await
        .expect("file cache token access");

    assert_eq!(cached.as_str(), "persisted-token");
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn token_provider_file_cache_is_scoped_by_account() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"account-a-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"account-b-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_path = tempdir.path().join("kis-token.json");
    let account_a = AccountKey::new("12345678-01").expect("account key");
    let account_b = AccountKey::new("87654321-01").expect("account key");
    let provider_a = token_provider_with_account(
        BrokerEnvironment::Paper,
        server.base_url(),
        Some(cache_path.clone()),
        Some(account_a),
    );
    let provider_b = token_provider_with_account(
        BrokerEnvironment::Paper,
        server.base_url(),
        Some(cache_path),
        Some(account_b),
    );

    let token_a = provider_a
        .access_token(false)
        .await
        .expect("account A token");
    let token_b = provider_b
        .access_token(false)
        .await
        .expect("account B token");

    assert_eq!(token_a.as_str(), "account-a-token");
    assert_eq!(token_b.as_str(), "account-b-token");
    assert_eq!(server.requests().len(), 2);
}

#[tokio::test]
async fn token_provider_file_lock_coalesces_cross_provider_refreshes() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"locked-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_path = tempdir.path().join("kis-token.json");
    let first = token_provider(
        BrokerEnvironment::Paper,
        server.base_url(),
        Some(cache_path.clone()),
    );
    let second = token_provider(
        BrokerEnvironment::Paper,
        server.base_url(),
        Some(cache_path),
    );

    let (first, second) = tokio::join!(first.access_token(false), second.access_token(false));

    assert_eq!(first.expect("first token").as_str(), "locked-token");
    assert_eq!(second.expect("second token").as_str(), "locked-token");
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn token_provider_recovers_stale_file_refresh_lock() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"recovered-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_path = tempdir.path().join("kis-token.json");
    let lock_path = tempdir.path().join("kis-token.json.lock");
    std::fs::write(&lock_path, "stale-pid").expect("write stale lock");
    let provider = token_provider_with_lock_policy(
        BrokerEnvironment::Paper,
        server.base_url(),
        cache_path,
        Duration::ZERO,
        Duration::from_secs(1),
    );

    let token = provider
        .access_token(false)
        .await
        .expect("stale lock should be recovered");

    assert_eq!(token.as_str(), "recovered-token");
    assert_eq!(server.requests().len(), 1);
    assert!(!lock_path.exists());
}

#[tokio::test]
async fn token_provider_times_out_waiting_for_fresh_file_refresh_lock() {
    let server = FakeKisServer::start();
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_path = tempdir.path().join("kis-token.json");
    let lock_path = tempdir.path().join("kis-token.json.lock");
    std::fs::write(&lock_path, std::process::id().to_string()).expect("write active lock");
    let provider = token_provider_with_lock_policy(
        BrokerEnvironment::Paper,
        server.base_url(),
        cache_path,
        Duration::from_secs(60),
        Duration::from_millis(1),
    );

    let error = provider
        .access_token(false)
        .await
        .expect_err("fresh lock should time out instead of hanging");

    assert!(error.to_string().contains("timed out"));
    assert!(server.requests().is_empty());
}

#[tokio::test]
async fn token_provider_retries_temporary_issue_failure_once() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        503,
        r#"{"rt_cd":"1","msg_cd":"EGW001","msg1":"temporary"}"#,
    ));
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"retried-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    let provider = token_provider(BrokerEnvironment::Paper, server.base_url(), None);

    let token = provider.access_token(false).await.expect("retry succeeds");

    assert_eq!(token.as_str(), "retried-token");
    assert_eq!(server.requests().len(), 2);
}

#[cfg(unix)]
#[tokio::test]
async fn token_provider_writes_cache_with_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"permission-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_path = tempdir.path().join("kis-token.json");
    let provider = token_provider(
        BrokerEnvironment::Paper,
        server.base_url(),
        Some(cache_path.clone()),
    );

    provider
        .access_token(false)
        .await
        .expect("token issue writes cache");

    let mode = std::fs::metadata(cache_path)
        .expect("cache metadata")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);
}

#[tokio::test]
async fn token_provider_discards_corrupt_cache_and_reissues() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"reissued-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    let tempdir = tempfile::tempdir().expect("tempdir");
    let cache_path = tempdir.path().join("kis-token.json");
    std::fs::write(&cache_path, "{not-json").expect("write corrupt cache");
    let provider = token_provider(
        BrokerEnvironment::Paper,
        server.base_url(),
        Some(cache_path),
    );

    let token = provider
        .access_token(false)
        .await
        .expect("token reissued after corrupt cache");

    assert_eq!(token.as_str(), "reissued-token");
    assert_eq!(server.requests().len(), 1);
}

#[tokio::test]
async fn token_provider_coalesces_concurrent_refreshes() {
    let server = FakeKisServer::start();
    server.push_response(FakeResponse::json(
        200,
        r#"{
            "access_token":"shared-token",
            "token_type":"Bearer",
            "expires_in":86400,
            "access_token_token_expired":"2099-01-01 09:00:00"
        }"#,
    ));
    let provider = Arc::new(token_provider(
        BrokerEnvironment::Paper,
        server.base_url(),
        None,
    ));
    let first = Arc::clone(&provider);
    let second = Arc::clone(&provider);

    let (first, second) = tokio::join!(first.access_token(false), second.access_token(false));

    assert_eq!(first.expect("first token").as_str(), "shared-token");
    assert_eq!(second.expect("second token").as_str(), "shared-token");
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn live_token_provider_requires_file_cache() {
    let credential = KisCredential::new("live-app-key", "live-app-secret").expect("credential");
    let config = KisAuthConfig::new(BrokerEnvironment::Live, credential).expect("auth config");

    let error = KisTokenProvider::new(config).expect_err("live provider requires cache");

    assert!(
        error
            .to_string()
            .contains("live token provider requires file cache")
    );
}

fn token_provider(
    environment: BrokerEnvironment,
    base_url: &str,
    cache_path: Option<std::path::PathBuf>,
) -> KisTokenProvider {
    token_provider_with_account(environment, base_url, cache_path, None)
}

fn token_provider_with_account(
    environment: BrokerEnvironment,
    base_url: &str,
    cache_path: Option<std::path::PathBuf>,
    account_key: Option<AccountKey>,
) -> KisTokenProvider {
    let credential = KisCredential::new("app-key", "app-secret").expect("credential");
    let mut config = KisAuthConfig::new(environment, credential)
        .expect("auth config")
        .with_base_url(base_url);
    if let Some(cache_path) = cache_path {
        config = config.with_cache_path(cache_path);
    }
    if let Some(account_key) = account_key {
        config = config.with_account_key(account_key);
    }
    KisTokenProvider::new(config).expect("token provider")
}

fn token_provider_with_lock_policy(
    environment: BrokerEnvironment,
    base_url: &str,
    cache_path: std::path::PathBuf,
    stale_after: Duration,
    acquire_timeout: Duration,
) -> KisTokenProvider {
    let credential = KisCredential::new("app-key", "app-secret").expect("credential");
    let config = KisAuthConfig::new(environment, credential)
        .expect("auth config")
        .with_base_url(base_url)
        .with_cache_path(cache_path)
        .with_token_lock_policy(stale_after, acquire_timeout)
        .expect("token lock policy");
    KisTokenProvider::new(config).expect("token provider")
}
