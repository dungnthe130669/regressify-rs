use base64::{Engine as _, engine::general_purpose};
use sha2::{Digest, Sha384};

pub fn get_asset_version(content: &[u8]) -> String {
    let mut hasher = Sha384::new();
    hasher.update(content);
    let result = hasher.finalize();
    // Node's 'base64url' (URL-safe, no padding)
    let encoded = general_purpose::URL_SAFE_NO_PAD.encode(result);
    encoded.chars().take(10).collect()
}
