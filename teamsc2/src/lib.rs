// Declare our modules
mod auth;
mod teamsc2;
mod config;
mod named_pipes;
mod teamsclient;

// Re-export types and functions that we want to be publicly available
// This is called the public API of our library
pub use teamsc2::{TeamsC2, Config};
pub use config::*;