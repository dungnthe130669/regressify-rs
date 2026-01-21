#![allow(dead_code)]

pub fn format_bytes<T: Into<u64>>(bytes: T) -> String {
    let bytes = bytes.into();
    const UNITS: [&str; 7] = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1000.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{}{}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.2}{}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0u64), "0B");
        assert_eq!(format_bytes(10u64), "10B");
        assert_eq!(format_bytes(500u64), "500B");
        assert_eq!(format_bytes(999u64), "999B");
        assert_eq!(format_bytes(1000u64), "0.98KB");
        assert_eq!(format_bytes(1023u64), "1.00KB");
        assert_eq!(format_bytes(1024u64), "1.00KB");
        assert_eq!(format_bytes(1500u64), "1.46KB");
        assert_eq!(format_bytes(2048u64), "2.00KB");
        assert_eq!(format_bytes(5_242_880u64), "5.00MB");
        assert_eq!(format_bytes(10_737_418_240u64), "10.00GB");
        assert_eq!(format_bytes(1_099_511_627_776u64), "1.00TB");
        assert_eq!(format_bytes(1_125_899_906_842_624u64), "1.00PB");
        assert_eq!(format_bytes(18_446_744_073_709_551_615u64), "16.00EB");
    }
}
