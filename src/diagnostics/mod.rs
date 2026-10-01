//! Versioned, safe execution diagnostics. Progress remains a separate live protocol;
//! this module owns durable records and the one-way presentation adapters.

pub mod adapter;
pub mod record;
pub(crate) mod redaction;
