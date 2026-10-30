#![forbid(unsafe_code)]

pub mod advisor;
pub mod backtest;
pub mod command_usage;
pub mod config;
pub mod decision;
pub mod doctor;
pub mod error;
pub mod execution;
pub mod feedback;
pub mod gateway;
pub mod market;
pub mod market_alignment;
pub(crate) mod order_price;
pub mod reconciliation;
pub mod risk;
pub mod runner;
pub mod runtime;
pub mod stdout;
pub mod storage;
pub mod strategy;
pub mod technical_analysis;
pub mod universe;
