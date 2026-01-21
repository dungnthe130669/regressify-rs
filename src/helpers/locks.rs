#![allow(dead_code)]

use std::sync::OnceLock;

use anyhow::{Result, anyhow};

pub trait OnceLockExtensions<T> {
    fn get_value(&self) -> Option<&T>;
    fn get_required(&self) -> Result<&T>;
    fn set_value(&self, value: T) -> Result<()>;
}

pub trait OnceLockBoolExtensions {
    fn get_bool(&self) -> bool;
    fn set_bool(&self, value: bool) -> Result<()>;
}

impl<T> OnceLockExtensions<T> for OnceLock<T> {
    fn get_value(&self) -> Option<&T> {
        self.get()
    }

    fn get_required(&self) -> Result<&T> {
        self.get().ok_or_else(|| anyhow!("Value not set"))
    }

    fn set_value(&self, value: T) -> Result<()> {
        self.set(value).map_err(|_| anyhow!("Failed to set value"))
    }
}

impl OnceLockBoolExtensions for OnceLock<bool> {
    fn set_bool(&self, value: bool) -> Result<()> {
        self.set(value).map_err(|_| anyhow!("Failed to set value"))
    }

    fn get_bool(&self) -> bool {
        *self.get_value().unwrap_or(&false)
    }
}
