use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use tokio::sync::Mutex;
use tokio::task;
use tokio::time::{Instant, sleep_until};

use super::{EndpointGroup, KisRateLimitConfig};
use crate::core::BrokerError;
use crate::domain::BrokerEnvironment;

#[derive(Debug)]
pub(super) struct KisRateLimiter {
    config: KisRateLimitConfig,
    global_next_available_at: Mutex<Option<Instant>>,
    next_available_at: Mutex<BTreeMap<EndpointGroup, Instant>>,
    circuit_open_until: Mutex<BTreeMap<EndpointGroup, Instant>>,
    shared: Option<SharedRateLimiter>,
}

#[derive(Debug, Clone)]
pub(super) struct SharedRateLimitConfig {
    path: PathBuf,
    namespace: String,
}

impl SharedRateLimitConfig {
    pub(super) fn new(path: PathBuf, environment: BrokerEnvironment, app_key: &str) -> Self {
        Self {
            path,
            namespace: format!(
                "{}:{}",
                environment.as_str(),
                stable_secret_fingerprint(app_key)
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct SharedRateLimiter {
    config: SharedRateLimitConfig,
}

impl KisRateLimiter {
    pub(super) fn new(config: KisRateLimitConfig, shared: Option<SharedRateLimitConfig>) -> Self {
        Self {
            config,
            global_next_available_at: Mutex::new(None),
            next_available_at: Mutex::new(BTreeMap::new()),
            circuit_open_until: Mutex::new(BTreeMap::new()),
            shared: shared.map(|config| SharedRateLimiter { config }),
        }
    }

    pub(super) async fn wait(&self, group: EndpointGroup) -> Result<(), BrokerError> {
        if let Some(shared) = &self.shared {
            let wait_for = shared.reserve_wait(self.config, group).await?;
            if !wait_for.is_zero() {
                tokio::time::sleep(wait_for).await;
            }
            return Ok(());
        }

        let now = Instant::now();
        let circuit_until = {
            let circuit_open_until = self.circuit_open_until.lock().await;
            circuit_open_until.get(&group).copied()
        };
        if let Some(circuit_until) = circuit_until
            && circuit_until > now
        {
            sleep_until(circuit_until).await;
        }

        self.wait_global_interval().await;

        let min_interval = self.config.min_interval(group);
        if min_interval.is_zero() {
            return Ok(());
        }
        let now = Instant::now();
        let wait_until = {
            let mut next_available_at = self.next_available_at.lock().await;
            let available_at = *next_available_at.get(&group).unwrap_or(&now);
            let scheduled_at = if available_at > now {
                available_at
            } else {
                now
            };
            next_available_at.insert(group, scheduled_at + min_interval);
            scheduled_at
        };

        if wait_until > now {
            sleep_until(wait_until).await;
        }
        Ok(())
    }

    async fn wait_global_interval(&self) {
        let min_interval = self.config.global_min_interval;
        if min_interval.is_zero() {
            return;
        }

        let now = Instant::now();
        let wait_until = {
            let mut global_next_available_at = self.global_next_available_at.lock().await;
            let available_at = global_next_available_at.unwrap_or(now);
            let scheduled_at = if available_at > now {
                available_at
            } else {
                now
            };
            *global_next_available_at = Some(scheduled_at + min_interval);
            scheduled_at
        };

        if wait_until > now {
            sleep_until(wait_until).await;
        }
    }

    pub(super) async fn record_throttle(&self, group: EndpointGroup) {
        let open_for = self.config.throttle_circuit_open_for;
        if open_for.is_zero() {
            return;
        }

        if let Some(shared) = &self.shared {
            if let Err(error) = shared.record_throttle(group, open_for).await {
                tracing::warn!(
                    event = "kis_rate_limit",
                    mode = "shared",
                    group = group.as_str(),
                    error = %error,
                    "failed to record shared KIS throttle circuit"
                );
            }
            return;
        }

        let mut circuit_open_until = self.circuit_open_until.lock().await;
        circuit_open_until.insert(group, Instant::now() + open_for);
    }
}

impl SharedRateLimiter {
    async fn reserve_wait(
        &self,
        config: KisRateLimitConfig,
        group: EndpointGroup,
    ) -> Result<Duration, BrokerError> {
        let shared = self.config.clone();
        task::spawn_blocking(move || reserve_shared_wait(shared, config, group))
            .await
            .map_err(|error| {
                BrokerError::persistence(format!("shared KIS rate limiter task failed: {error}"))
            })?
    }

    async fn record_throttle(
        &self,
        group: EndpointGroup,
        open_for: Duration,
    ) -> Result<(), BrokerError> {
        let shared = self.config.clone();
        task::spawn_blocking(move || record_shared_throttle(shared, group, open_for))
            .await
            .map_err(|error| {
                BrokerError::persistence(format!("shared KIS throttle task failed: {error}"))
            })?
    }
}

fn reserve_shared_wait(
    shared: SharedRateLimitConfig,
    config: KisRateLimitConfig,
    group: EndpointGroup,
) -> Result<Duration, BrokerError> {
    let now_ms = now_millis()?;
    let group_key = shared_group_key(&shared.namespace, group);
    let global_key = shared_global_key(&shared.namespace);
    let mut connection = open_shared_connection(&shared.path)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_error)?;
    ensure_shared_schema(&transaction)?;

    let mut wait_until_ms = now_ms;
    if let Some(circuit_open_until_ms) = read_circuit_open_until(&transaction, &group_key)?
        && circuit_open_until_ms > wait_until_ms
    {
        wait_until_ms = circuit_open_until_ms;
    }

    wait_until_ms = reserve_shared_key(
        &transaction,
        &global_key,
        wait_until_ms,
        duration_millis(config.global_min_interval),
    )?;
    wait_until_ms = reserve_shared_key(
        &transaction,
        &group_key,
        wait_until_ms,
        duration_millis(config.min_interval(group)),
    )?;

    transaction.commit().map_err(sqlite_error)?;
    let wait_ms = wait_until_ms.saturating_sub(now_ms);
    Ok(Duration::from_millis(wait_ms))
}

fn record_shared_throttle(
    shared: SharedRateLimitConfig,
    group: EndpointGroup,
    open_for: Duration,
) -> Result<(), BrokerError> {
    let now_ms = now_millis()?;
    let circuit_open_until_ms = now_ms.saturating_add(duration_millis(open_for));
    let group_key = shared_group_key(&shared.namespace, group);
    let mut connection = open_shared_connection(&shared.path)?;
    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sqlite_error)?;
    ensure_shared_schema(&transaction)?;
    transaction
        .execute(
            "INSERT INTO kis_rate_limit_state
                 (key, next_available_at_ms, circuit_open_until_ms)
             VALUES (?1, 0, ?2)
             ON CONFLICT(key) DO UPDATE SET
                 circuit_open_until_ms = MAX(circuit_open_until_ms, excluded.circuit_open_until_ms)",
            params![group_key, circuit_open_until_ms],
        )
        .map_err(sqlite_error)?;
    transaction.commit().map_err(sqlite_error)?;
    Ok(())
}

fn open_shared_connection(path: &PathBuf) -> Result<Connection, BrokerError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            BrokerError::persistence(format!(
                "failed to create shared KIS rate limit directory {}: {error}",
                parent.display()
            ))
        })?;
    }

    let connection = Connection::open(path).map_err(sqlite_error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(sqlite_error)?;
    Ok(connection)
}

fn ensure_shared_schema(connection: &Connection) -> Result<(), BrokerError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS kis_rate_limit_state (
                 key TEXT PRIMARY KEY,
                 next_available_at_ms INTEGER NOT NULL,
                 circuit_open_until_ms INTEGER NOT NULL
             );",
        )
        .map_err(sqlite_error)
}

fn read_circuit_open_until(connection: &Connection, key: &str) -> Result<Option<u64>, BrokerError> {
    connection
        .query_row(
            "SELECT circuit_open_until_ms
               FROM kis_rate_limit_state
              WHERE key = ?1",
            params![key],
            |row| row.get::<_, u64>(0),
        )
        .optional()
        .map_err(sqlite_error)
}

fn reserve_shared_key(
    connection: &Connection,
    key: &str,
    earliest_ms: u64,
    min_interval_ms: u64,
) -> Result<u64, BrokerError> {
    if min_interval_ms == 0 {
        return Ok(earliest_ms);
    }

    let current_next_available_ms = connection
        .query_row(
            "SELECT next_available_at_ms
               FROM kis_rate_limit_state
              WHERE key = ?1",
            params![key],
            |row| row.get::<_, u64>(0),
        )
        .optional()
        .map_err(sqlite_error)?
        .unwrap_or(0);
    let scheduled_ms = earliest_ms.max(current_next_available_ms);
    let next_available_ms = scheduled_ms.saturating_add(min_interval_ms);
    connection
        .execute(
            "INSERT INTO kis_rate_limit_state
                 (key, next_available_at_ms, circuit_open_until_ms)
             VALUES (?1, ?2, 0)
             ON CONFLICT(key) DO UPDATE SET
                 next_available_at_ms = excluded.next_available_at_ms",
            params![key, next_available_ms],
        )
        .map_err(sqlite_error)?;
    Ok(scheduled_ms)
}

fn now_millis() -> Result<u64, BrokerError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            BrokerError::persistence(format!("system clock is before UNIX epoch: {error}"))
        })?;
    Ok(duration_millis(elapsed))
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn shared_global_key(namespace: &str) -> String {
    format!("{namespace}:global")
}

fn shared_group_key(namespace: &str, group: EndpointGroup) -> String {
    format!("{namespace}:group:{}", group.as_str())
}

fn stable_secret_fingerprint(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn sqlite_error(error: rusqlite::Error) -> BrokerError {
    BrokerError::persistence(format!("shared KIS rate limiter SQLite error: {error}"))
}
