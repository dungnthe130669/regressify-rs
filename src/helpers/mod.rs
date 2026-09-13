#![allow(unused_imports)]

mod asset;
mod command;
mod date_time;
mod http;
mod io;
mod locks;
mod numbers;
pub(crate) mod png;
mod slugify;

#[cfg(unix)]
mod mode;

pub use asset::AssetExtensions;
pub use command::CommandExtensions;
pub use date_time::{NaiveTimeExtensions, elapsed_time, sleep_remaining_time};
pub use http::HttpExtensions;
pub use io::{
    IOExtensions, copy_file_atomically, format_size, is_windows_reserved_path_component,
    write_file_atomically,
};
pub use locks::{OnceLockBoolExtensions, OnceLockExtensions};
pub use numbers::format_bytes;
pub use slugify::slugify;

#[cfg(unix)]
pub use mode::cmod;
#[cfg(unix)]
pub use mode::set_unix_mode;
