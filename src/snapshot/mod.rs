mod actions;
mod capture_report;
mod helpers;
mod site;

pub use actions::snapshot;
pub use helpers::get_asset_version;
#[cfg(all(test, feature = "desktop"))]
pub(crate) use site::generate_site;
#[cfg(feature = "desktop")]
pub(crate) use site::update_suite;
#[cfg(feature = "desktop")]
pub(crate) use site::{
    FRONTEND_FILES, ImageRole, artifact_paths, image_roles, refresh_site_versions,
};
