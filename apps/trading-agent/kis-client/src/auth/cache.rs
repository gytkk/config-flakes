use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use time::OffsetDateTime;

use crate::BrokerEnvironment;
use crate::core::BrokerError;

use super::config::KisAuthConfig;
use super::token::{AccessToken, TokenSnapshot};

const TOKEN_CACHE_SCHEMA_VERSION: u16 = 2;

pub(super) fn read_file_cache(
    config: &KisAuthConfig,
) -> Result<Option<TokenSnapshot>, BrokerError> {
    let Some(cache_path) = &config.cache_path else {
        return Ok(None);
    };

    let metadata = match fs::metadata(cache_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(BrokerError::persistence(format!(
                "failed to inspect KIS token cache: {error}"
            )));
        }
    };

    if !metadata.is_file() || !owner_only_permissions(&metadata) {
        return Ok(None);
    }

    let body = match fs::read_to_string(cache_path) {
        Ok(body) => body,
        Err(error) if error.kind() == ErrorKind::InvalidData => return Ok(None),
        Err(error) => {
            return Err(BrokerError::persistence(format!(
                "failed to read KIS token cache: {error}"
            )));
        }
    };

    let payload: TokenCachePayload = match serde_json::from_str(&body) {
        Ok(payload) => payload,
        Err(_) => return Ok(None),
    };
    if payload.schema_version != TOKEN_CACHE_SCHEMA_VERSION
        || payload.environment != config.environment
        || payload.app_key_fingerprint != app_key_fingerprint(config.credential.app_key())
        || payload.account_key_fingerprint.as_deref()
            != config
                .account_key
                .as_ref()
                .map(|account_key| account_key_fingerprint(account_key.as_str()))
                .as_deref()
    {
        return Ok(None);
    }

    Ok(Some(TokenSnapshot {
        access_token: AccessToken::new(payload.access_token)?,
        token_type: payload.token_type,
        expires_in_seconds: payload.expires_in_seconds,
        expires_at_utc: payload.expires_at_utc,
    }))
}

pub(super) fn write_file_cache(
    config: &KisAuthConfig,
    snapshot: &TokenSnapshot,
) -> Result<(), BrokerError> {
    let Some(cache_path) = &config.cache_path else {
        return Ok(());
    };

    let payload = TokenCachePayload {
        schema_version: TOKEN_CACHE_SCHEMA_VERSION,
        environment: config.environment,
        account_key_fingerprint: config
            .account_key
            .as_ref()
            .map(|account_key| account_key_fingerprint(account_key.as_str())),
        app_key_fingerprint: app_key_fingerprint(config.credential.app_key()),
        token_type: snapshot.token_type.clone(),
        access_token: snapshot.access_token.as_str().to_owned(),
        expires_in_seconds: snapshot.expires_in_seconds,
        expires_at_utc: snapshot.expires_at_utc,
    };
    let encoded = serde_json::to_vec_pretty(&payload).map_err(|error| {
        BrokerError::persistence(format!("failed to encode KIS token cache: {error}"))
    })?;

    atomic_write_owner_only(cache_path, &encoded)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TokenCachePayload {
    schema_version: u16,
    environment: BrokerEnvironment,
    account_key_fingerprint: Option<String>,
    app_key_fingerprint: String,
    token_type: String,
    access_token: String,
    expires_in_seconds: u64,
    expires_at_utc: OffsetDateTime,
}

fn owner_only_permissions(metadata: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        metadata.permissions().mode() & 0o777 == 0o600
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn atomic_write_owner_only(path: &Path, body: &[u8]) -> Result<(), BrokerError> {
    let parent = path.parent().ok_or_else(|| {
        BrokerError::persistence("KIS token cache path must have a parent directory")
    })?;
    fs::create_dir_all(parent).map_err(|error| {
        BrokerError::persistence(format!(
            "failed to create KIS token cache directory: {error}"
        ))
    })?;

    let temp_path = temp_cache_path(path);
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    let mut file = options.open(&temp_path).map_err(|error| {
        BrokerError::persistence(format!("failed to open KIS token cache temp file: {error}"))
    })?;
    file.write_all(body).map_err(|error| {
        BrokerError::persistence(format!("failed to write KIS token cache: {error}"))
    })?;
    file.sync_all().map_err(|error| {
        BrokerError::persistence(format!("failed to sync KIS token cache: {error}"))
    })?;
    drop(file);

    #[cfg(unix)]
    fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        BrokerError::persistence(format!(
            "failed to set KIS token cache permissions: {error}"
        ))
    })?;

    fs::rename(&temp_path, path).map_err(|error| {
        BrokerError::persistence(format!("failed to replace KIS token cache: {error}"))
    })?;

    Ok(())
}

fn temp_cache_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("kis-token-cache");
    path.with_file_name(format!(".{file_name}.{}.tmp", std::process::id()))
}

fn app_key_fingerprint(app_key: &str) -> String {
    fnv1a_fingerprint(app_key)
}

fn account_key_fingerprint(account_key: &str) -> String {
    fnv1a_fingerprint(account_key)
}

fn fnv1a_fingerprint(value: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}
