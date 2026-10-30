use serde::Deserialize;

pub mod domestic_stock;
pub mod overseas_stock;

pub use domestic_stock::*;
pub use overseas_stock::*;

#[derive(Debug, Clone, Deserialize)]
pub struct KisApiEnvelope<T> {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    pub output: Option<T>,
}
