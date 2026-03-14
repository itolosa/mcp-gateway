#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cognitive_complexity
)]

mod common;

mod adapters_driven_test {
    pub mod cli_execution;
    pub mod configuration;
    pub mod mcp_protocol;
    pub mod oauth;
    pub mod storage;
}
