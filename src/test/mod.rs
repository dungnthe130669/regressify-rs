pub mod actionability;
mod browsers;
pub mod capture;
mod page_stability;
mod test_action;
mod test_scenario;
mod test_suite;
mod test_suites;

pub use browsers::fetcher;
pub use test_suites::exec_ref;
pub use test_suites::exec_test_suites;

#[cfg(feature = "desktop")]
pub(crate) use test_suites::discover_test_suites;
