#[cfg(not(any(target_os = "linux", target_os = "windows")))]
compile_error!("coding-tools currently supports Linux and Windows only");

mod async_runtime;
pub mod admin;
mod actions;
pub mod activity;
mod app_state;
mod auth;
mod data;
mod error;
pub mod gateway;
pub mod harness;
pub mod headless;
mod health;
mod mcp;
mod platform;
mod runtime;
mod secret;
mod settings;
pub mod tools;
mod tunnel;
mod workspace;
