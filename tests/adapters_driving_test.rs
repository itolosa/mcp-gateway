#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod common;

mod adapters_driving_test {
    pub mod process;
    pub mod ui_command;
    pub mod ui_runner;
}
