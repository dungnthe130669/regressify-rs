mod actions;
mod capture_report;
mod helpers;
mod site;

pub use actions::snapshot;
pub use helpers::get_asset_version;
#[cfg(all(test, feature = "desktop"))]
pub(crate) use site::{FRONTEND_FILES, generate_site};
#[cfg(feature = "desktop")]
pub(crate) use site::{ImageRole, artifact_paths, image_roles, refresh_site_versions};
