use trading_agent_kis_client::domain::{AccountKey, BrokerEnvironment};
use trading_agent_kis_client::error::BrokerError;
use trading_agent_kis_client::safety::{
    SingleWriterLock, SingleWriterLockConfig, SingleWriterLockScope,
};

#[test]
fn single_writer_lock_blocks_same_environment_account() {
    let lock_dir = tempfile::tempdir().expect("temp lock dir");
    let config = SingleWriterLockConfig {
        lock_dir: lock_dir.path().to_path_buf(),
        environment: BrokerEnvironment::Live,
        account_key: AccountKey::new("12345678-01").expect("valid account key"),
        scope: SingleWriterLockScope::DomesticStock,
    };

    let first_lock = SingleWriterLock::acquire(config.clone()).expect("first lock succeeds");
    let error = SingleWriterLock::acquire(config).expect_err("second lock should be blocked");

    assert!(matches!(error, BrokerError::SafetyHalt { .. }));

    drop(first_lock);
}

#[test]
fn single_writer_lock_is_scoped_by_environment_account_and_market_scope() {
    let lock_dir = tempfile::tempdir().expect("temp lock dir");
    let live_config = SingleWriterLockConfig {
        lock_dir: lock_dir.path().to_path_buf(),
        environment: BrokerEnvironment::Live,
        account_key: AccountKey::new("12345678-01").expect("valid account key"),
        scope: SingleWriterLockScope::DomesticStock,
    };
    let overseas_config = SingleWriterLockConfig {
        lock_dir: lock_dir.path().to_path_buf(),
        environment: BrokerEnvironment::Live,
        account_key: AccountKey::new("12345678-01").expect("valid account key"),
        scope: SingleWriterLockScope::OverseasStock,
    };
    let paper_config = SingleWriterLockConfig {
        lock_dir: lock_dir.path().to_path_buf(),
        environment: BrokerEnvironment::Paper,
        account_key: AccountKey::new("12345678-01").expect("valid account key"),
        scope: SingleWriterLockScope::DomesticStock,
    };
    let other_account_config = SingleWriterLockConfig {
        lock_dir: lock_dir.path().to_path_buf(),
        environment: BrokerEnvironment::Live,
        account_key: AccountKey::new("87654321-01").expect("valid account key"),
        scope: SingleWriterLockScope::DomesticStock,
    };

    let _live_lock = SingleWriterLock::acquire(live_config.clone()).expect("live lock succeeds");
    let duplicate_live_lock = SingleWriterLock::acquire(live_config)
        .expect_err("same environment, account, and market scope should be blocked");
    let _paper_lock = SingleWriterLock::acquire(paper_config).expect("paper lock succeeds");
    let _overseas_lock =
        SingleWriterLock::acquire(overseas_config).expect("overseas lock succeeds");
    let _other_account_lock =
        SingleWriterLock::acquire(other_account_config).expect("other account lock succeeds");

    assert!(matches!(
        duplicate_live_lock,
        BrokerError::SafetyHalt { .. }
    ));
}

#[test]
fn single_writer_lock_releases_file_on_drop() {
    let lock_dir = tempfile::tempdir().expect("temp lock dir");
    let config = SingleWriterLockConfig {
        lock_dir: lock_dir.path().to_path_buf(),
        environment: BrokerEnvironment::Live,
        account_key: AccountKey::new("12345678-01").expect("valid account key"),
        scope: SingleWriterLockScope::DomesticStock,
    };

    {
        let _lock = SingleWriterLock::acquire(config.clone()).expect("first lock succeeds");
    }

    let _lock = SingleWriterLock::acquire(config).expect("lock can be reacquired after drop");
}
