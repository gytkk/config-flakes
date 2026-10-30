pub mod auth;
pub mod client;
pub mod config;

mod core;

pub mod domain {
    pub use crate::core::domain::*;
}

pub mod error {
    pub use crate::core::error::*;
}

pub mod journal {
    pub use crate::core::journal::*;
}

#[cfg_attr(
    not(feature = "internal-test-support"),
    allow(dead_code, unused_imports)
)]
mod mapping;
#[cfg_attr(not(feature = "internal-test-support"), allow(dead_code))]
mod protocol;
#[cfg_attr(not(feature = "internal-test-support"), allow(dead_code))]
mod reconcile;

#[cfg(feature = "internal-test-support")]
#[doc(hidden)]
pub mod internal {
    pub mod mapping {
        pub use crate::mapping::*;
    }

    pub mod protocol {
        pub use crate::protocol::*;
    }

    pub mod reconcile {
        pub use crate::reconcile::*;
    }
}

pub mod safety;
pub mod transport;

pub use auth::{KisAuthConfig, KisCredential, KisTokenProvider};
pub use client::{KisBrokerClient, KisClientBuilder};
pub use core::{BrokerClient, BrokerError};
pub use domain::*;
