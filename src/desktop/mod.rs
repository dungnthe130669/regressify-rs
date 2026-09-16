//! Headless desktop models. GPUI is only needed by the optional desktop binary.

pub mod completion;
pub mod document;
pub mod results;
pub mod runner;
pub mod schema;
pub mod session;
pub mod worker;
pub mod workspace;

#[cfg(feature = "gui")]
pub mod ui;
