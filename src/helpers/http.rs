#![allow(dead_code)]
use crate::common;
use reqwest::ClientBuilder;

use super::OnceLockExtensions;

pub trait HttpExtensions {
    fn set_optional_proxy(self) -> Self;
}

impl HttpExtensions for ClientBuilder {
    fn set_optional_proxy(self) -> Self {
        if let Some(proxy) = common::PROXY.get_value() {
            self.proxy(proxy.clone())
        } else {
            self
        }
    }
}
