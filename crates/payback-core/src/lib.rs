//! Payback core: loans, payments, balances, statements, data-file sharing
//! and backups. Depends only on the Rust standard library, so it builds the
//! same on macOS and Windows.

pub mod api;
pub mod backup;
pub mod csv;
pub mod date;
pub mod error;
pub mod id;
pub mod json;
pub mod ledger;
pub mod model;
pub mod money;
pub mod pdf;
mod pdf_metrics;
pub mod statement;
pub mod store;
pub mod transfer;

pub use api::Api;
pub use error::{Error, Result};
pub use json::Json;
pub use store::Store;
