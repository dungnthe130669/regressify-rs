pub mod actionability;
mod authentication;
mod browsers;
pub mod capture;
mod cdp;
mod hooks;
mod keyboard;
mod node_hook;
mod page_stability;
pub mod runtime;
mod storage_state;
mod tabs;
mod test_action;
mod test_scenario;
mod test_suite;
mod test_suites;

#[cfg(test)]
pub(crate) mod browser_tests;

pub use browsers::fetcher;
pub use test_suites::exec_ref;
pub use test_suites::exec_test_suites;

#[cfg(feature = "desktop")]
pub(crate) use test_suites::discover_test_suites;
