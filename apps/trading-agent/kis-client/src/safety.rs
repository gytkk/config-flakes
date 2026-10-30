use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use crate::core::BrokerError;
use crate::{AccountKey, BrokerEnvironment};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SingleWriterLockConfig {
    pub lock_dir: PathBuf,
    pub environment: BrokerEnvironment,
    pub account_key: AccountKey,
    pub scope: SingleWriterLockScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleWriterLockScope {
    DomesticStock,
    OverseasStock,
}

impl SingleWriterLockScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DomesticStock => "domestic_stock",
            Self::OverseasStock => "overseas_stock",
        }
    }
}

impl fmt::Display for SingleWriterLockScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

pub struct SingleWriterLock {
    path: PathBuf,
    environment: BrokerEnvironment,
    account_key: AccountKey,
    scope: SingleWriterLockScope,
    _file: File,
}

pub fn single_writer_lock_active(config: &SingleWriterLockConfig) -> Result<bool, BrokerError> {
    let path = lock_path(config);
    path.try_exists().map_err(|error| {
        BrokerError::persistence(format!(
            "failed to inspect single-writer lock path {}: {error}",
            path.display()
        ))
    })
}

impl SingleWriterLock {
    pub fn acquire(config: SingleWriterLockConfig) -> Result<Self, BrokerError> {
        fs::create_dir_all(&config.lock_dir).map_err(|error| {
            BrokerError::persistence(format!("failed to create lock directory: {error}"))
        })?;

        let path = lock_path(&config);
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                return Err(BrokerError::safety_halt(format!(
                    "single-writer lock is already held for {} {} {}",
                    config.environment, config.account_key, config.scope
                )));
            }
            Err(error) => {
                return Err(BrokerError::persistence(format!(
                    "failed to acquire single-writer lock: {error}"
                )));
            }
        };

        write_lock_metadata(&mut file, &config)?;
        file.sync_all().map_err(|error| {
            BrokerError::persistence(format!("failed to sync single-writer lock file: {error}"))
        })?;

        Ok(Self {
            path,
            environment: config.environment,
            account_key: config.account_key,
            scope: config.scope,
            _file: file,
        })
    }

    pub fn environment(&self) -> BrokerEnvironment {
        self.environment
    }

    pub fn account_key(&self) -> &AccountKey {
        &self.account_key
    }

    pub fn scope(&self) -> SingleWriterLockScope {
        self.scope
    }
}

impl fmt::Debug for SingleWriterLock {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SingleWriterLock")
            .field("path", &self.path)
            .field("environment", &self.environment)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl Drop for SingleWriterLock {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_file(&self.path)
            && error.kind() != ErrorKind::NotFound
        {
            tracing::warn!(
                path = %self.path.display(),
                error = %error,
                "failed to remove single-writer lock file"
            );
        }
    }
}

fn lock_path(config: &SingleWriterLockConfig) -> PathBuf {
    let environment = config.environment.as_str();
    let account = sanitized_component(config.account_key.as_str());
    let scope = config.scope.as_str();
    config
        .lock_dir
        .join(format!("kis-{environment}-{account}-{scope}.lock"))
}

fn sanitized_component(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn write_lock_metadata(
    file: &mut File,
    config: &SingleWriterLockConfig,
) -> Result<(), BrokerError> {
    writeln!(file, "environment={}", config.environment)
        .and_then(|()| writeln!(file, "account_key={}", config.account_key))
        .and_then(|()| writeln!(file, "scope={}", config.scope))
        .and_then(|()| writeln!(file, "pid={}", std::process::id()))
        .map_err(|error| {
            BrokerError::persistence(format!("failed to write single-writer metadata: {error}"))
        })
}
