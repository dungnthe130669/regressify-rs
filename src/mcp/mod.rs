use anyhow::Result;

mod catalog;

pub fn catalog_json() -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(&catalog::build()?)?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn catalog_bytes_are_deterministic_and_versioned() {
        let bytes = super::catalog_json().unwrap();
        assert_eq!(bytes, super::catalog_json().unwrap());
        let catalog: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(catalog["regressifyVersion"], env!("CARGO_PKG_VERSION"));
    }
}
