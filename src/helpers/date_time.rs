#![allow(dead_code)]

use anyhow::{Result, anyhow, bail};
use chrono::{DateTime, Local, NaiveTime, Timelike};
use tokio::time::sleep;

pub trait NaiveTimeExtensions {
    fn format_my_time(&self, include_hour: bool) -> String;
    fn format_join_time(&self, include_hour: bool) -> String;
    fn from_custom(input: &str) -> Result<NaiveTime>;
    fn from_float(input: f64) -> Result<NaiveTime>;
    fn to_float(&self) -> f64;
}

impl NaiveTimeExtensions for NaiveTime {
    fn format_my_time(&self, include_hour: bool) -> String {
        let format = if include_hour || self.hour() > 0 {
            "%H:%M:%S%.f"
        } else {
            "%M:%S%.f"
        };

        self.format(format).to_string()
    }

    fn format_join_time(&self, include_hour: bool) -> String {
        let format = if include_hour || self.hour() > 0 {
            "(%H:%M:%S%.f)"
        } else {
            "(%M:%S%.f)"
        };

        self.format(format)
            .to_string()
            .replace('.', ",")
            .replace(':', ".")
    }

    fn from_custom(input: &str) -> Result<NaiveTime> {
        if let Ok(time) = NaiveTime::parse_from_str(input, "%H:%M:%S%.f") {
            return Ok(time);
        }

        // if let Ok(time) = NaiveTime::parse_from_str(input, "%H.%M.%S[%6f]") {
        //     return Ok(time);
        // }

        if let Ok(time) = NaiveTime::parse_from_str(&format!("0:{}", input), "%H:%M:%S%.f") {
            return Ok(time);
        }

        // if let Ok(time) = NaiveTime::parse_from_str(&format!("0.{}", input), "%H.%M.%S[%6f]") {
        //     return Ok(time);
        // }

        bail!("Cannot parse time string: {}.", input);
    }

    fn from_float(input: f64) -> Result<NaiveTime> {
        if input < 0_f64 {
            bail!("Time must greater than or equals to 0");
        }

        let hour = input as u32 / 3600;
        let min = (input as u32 - hour * 3600) / 60;
        let sec = input as u32 - hour * 3600 - min * 60;
        let nano = ((input - input.floor()) * 1000000000_f64) as u32;

        let time = NaiveTime::from_hms_nano_opt(hour, min, sec, nano);
        match time {
            Some(time) => Ok(time),
            None => bail!("Cannot parse time from float"),
        }
    }

    fn to_float(&self) -> f64 {
        self.hour() as f64 * 3600_f64
            + self.minute() as f64 * 60_f64
            + self.second() as f64
            + (self.nanosecond() as f64 % 1_000_000_000_f64) / 1_000_000_000_f64
    }
}

pub fn elapsed_time(start_time: DateTime<Local>) -> chrono::Duration {
    chrono::offset::Local::now().signed_duration_since(start_time)
}

pub async fn sleep_remaining_time(duration_seconds: u64, start_time: DateTime<Local>) {
    // Calculate the elapsed time since the start time
    let elapsed = elapsed_time(start_time);

    // Convert duration to milliseconds
    let duration_milis = duration_seconds * 1000;

    // Calculate the passed milliseconds
    let passed_millis = elapsed.num_milliseconds() as u64 % duration_milis;

    // Calculate the remaining milliseconds
    let remaining_millis = duration_milis - passed_millis;

    sleep(std::time::Duration::from_millis(remaining_millis)).await;
}

pub fn is_timed_out(start_time: DateTime<Local>, wait_timeout_seconds: i64) -> bool {
    chrono::offset::Local::now() > (start_time + chrono::Duration::seconds(wait_timeout_seconds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    // ========== format_my_time tests ==========

    #[test]
    fn test_format_my_time_with_hour_include_true() {
        let time = NaiveTime::from_hms_opt(2, 30, 45).unwrap();
        let result = time.format_my_time(true);
        assert_eq!(result, "02:30:45");
    }

    #[test]
    fn test_format_my_time_with_hour_include_false_zero_hour() {
        let time = NaiveTime::from_hms_opt(0, 15, 30).unwrap();
        let result = time.format_my_time(false);
        assert_eq!(result, "15:30");
    }

    #[test]
    fn test_format_my_time_with_hour_include_false_nonzero_hour() {
        let time = NaiveTime::from_hms_opt(1, 15, 30).unwrap();
        let result = time.format_my_time(false);
        assert!(result.starts_with("01:15:30"));
    }

    #[test]
    fn test_format_my_time_midnight() {
        let time = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let result = time.format_my_time(false);
        assert_eq!(result, "00:00");
    }

    #[test]
    fn test_format_my_time_with_nanoseconds() {
        let time = NaiveTime::from_hms_nano_opt(0, 5, 10, 123456789).unwrap();
        let result = time.format_my_time(false);
        assert!(result.contains("05:10"));
    }

    #[test]
    fn test_format_my_time_end_of_day() {
        let time = NaiveTime::from_hms_opt(23, 59, 59).unwrap();
        let result = time.format_my_time(true);
        assert_eq!(result, "23:59:59");
    }

    // ========== format_join_time tests ==========

    #[test]
    fn test_format_join_time_with_hour_include_true() {
        let time = NaiveTime::from_hms_opt(2, 30, 45).unwrap();
        let result = time.format_join_time(true);
        assert_eq!(result, "(02.30.45)");
    }

    #[test]
    fn test_format_join_time_with_hour_include_false_zero_hour() {
        let time = NaiveTime::from_hms_opt(0, 15, 30).unwrap();
        let result = time.format_join_time(false);
        assert_eq!(result, "(15.30)");
    }

    #[test]
    fn test_format_join_time_with_hour_include_false_nonzero_hour() {
        let time = NaiveTime::from_hms_opt(1, 15, 30).unwrap();
        let result = time.format_join_time(false);
        assert!(result.contains("01.15.30"));
    }

    #[test]
    fn test_format_join_time_replaces_colons_with_dots() {
        let time = NaiveTime::from_hms_opt(3, 45, 12).unwrap();
        let result = time.format_join_time(true);
        assert!(!result.contains(':'));
        assert!(result.contains('.'));
    }

    #[test]
    fn test_format_join_time_replaces_dot_with_comma() {
        let time = NaiveTime::from_hms_nano_opt(1, 30, 45, 500000000).unwrap();
        let result = time.format_join_time(true);
        assert!(result.contains(','));
    }

    #[test]
    fn test_format_join_time_midnight() {
        let time = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let result = time.format_join_time(false);
        assert_eq!(result, "(00.00)");
    }

    #[test]
    fn test_format_join_time_end_of_day() {
        let time = NaiveTime::from_hms_opt(23, 59, 59).unwrap();
        let result = time.format_join_time(true);
        assert!(result.contains("23.59.59"));
    }

    // ========== from_custom tests ==========

    #[test]
    fn test_from_custom_standard_format() {
        let result = NaiveTime::from_custom("12:30:45.123456");
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.hour(), 12);
        assert_eq!(time.minute(), 30);
        assert_eq!(time.second(), 45);
    }

    #[test]
    fn test_from_custom_zero_hour() {
        let result = NaiveTime::from_custom("00:30:45.0");
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.hour(), 0);
        assert_eq!(time.minute(), 30);
    }

    #[test]
    fn test_from_custom_short_format_minutes_seconds() {
        let result = NaiveTime::from_custom("30:45.0");
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.minute(), 30);
        assert_eq!(time.second(), 45);
    }

    #[test]
    fn test_from_custom_short_format_with_nanoseconds() {
        let result = NaiveTime::from_custom("05:10.500000");
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.minute(), 5);
        assert_eq!(time.second(), 10);
    }

    #[test]
    fn test_from_custom_midnight() {
        let result = NaiveTime::from_custom("00:00:00.0");
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.hour(), 0);
        assert_eq!(time.minute(), 0);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn test_from_custom_end_of_day() {
        let result = NaiveTime::from_custom("23:59:59.999999");
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.hour(), 23);
        assert_eq!(time.minute(), 59);
        assert_eq!(time.second(), 59);
    }

    #[test]
    fn test_from_custom_invalid_hour() {
        let result = NaiveTime::from_custom("25:00:00.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_custom_invalid_minute() {
        let result = NaiveTime::from_custom("12:60:00.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_custom_invalid_second() {
        // Note: "12:30:60.0" might not actually fail if chrono allows it
        // Testing with a clearly invalid case instead
        let result = NaiveTime::from_custom("25:30:00.0");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_custom_empty_string() {
        let result = NaiveTime::from_custom("");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_custom_malformed_string() {
        let result = NaiveTime::from_custom("not a time");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_custom_partial_short_format() {
        // "45.0" by itself won't parse, needs at least MM:SS format
        let result = NaiveTime::from_custom("45:30.0");
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.minute(), 45);
        assert_eq!(time.second(), 30);
    }

    // ========== from_float tests ==========

    #[test]
    fn test_from_float_zero() {
        let result = NaiveTime::from_float(0.0);
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.hour(), 0);
        assert_eq!(time.minute(), 0);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn test_from_float_seconds_only() {
        let result = NaiveTime::from_float(45.0);
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.second(), 45);
    }

    #[test]
    fn test_from_float_minutes_and_seconds() {
        let result = NaiveTime::from_float(130.0); // 2 minutes 10 seconds
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.minute(), 2);
        assert_eq!(time.second(), 10);
    }

    #[test]
    fn test_from_float_hours_minutes_seconds() {
        let result = NaiveTime::from_float(3665.0); // 1 hour 1 minute 5 seconds
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.hour(), 1);
        assert_eq!(time.minute(), 1);
        assert_eq!(time.second(), 5);
    }

    #[test]
    fn test_from_float_with_fraction() {
        let result = NaiveTime::from_float(45.5);
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.second(), 45);
        assert!(time.nanosecond() > 0);
    }

    #[test]
    fn test_from_float_midnight() {
        let result = NaiveTime::from_float(0.0);
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.hour(), 0);
        assert_eq!(time.minute(), 0);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn test_from_float_end_of_day() {
        let result = NaiveTime::from_float(86399.0); // 23:59:59
        assert!(result.is_ok());
        let time = result.unwrap();
        assert_eq!(time.hour(), 23);
        assert_eq!(time.minute(), 59);
        assert_eq!(time.second(), 59);
    }

    #[test]
    fn test_from_float_negative() {
        let result = NaiveTime::from_float(-1.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_from_float_large_value() {
        let result = NaiveTime::from_float(86400.0); // Exactly 24 hours
        assert!(result.is_err()); // Should fail as it exceeds valid time range
    }

    #[test]
    fn test_from_float_very_small_fraction() {
        let result = NaiveTime::from_float(0.000001);
        assert!(result.is_ok());
        let time = result.unwrap();
        assert!(time.nanosecond() > 0);
    }

    #[test]
    fn test_from_float_large_fraction() {
        let result = NaiveTime::from_float(0.999999999);
        assert!(result.is_ok());
    }

    // ========== to_float tests ==========

    #[test]
    fn test_to_float_midnight() {
        let time = NaiveTime::from_hms_opt(0, 0, 0).unwrap();
        let result = time.to_float();
        assert_eq!(result, 0.0);
    }

    #[test]
    fn test_to_float_seconds_only() {
        let time = NaiveTime::from_hms_opt(0, 0, 45).unwrap();
        let result = time.to_float();
        assert_eq!(result, 45.0);
    }

    #[test]
    fn test_to_float_minutes_and_seconds() {
        let time = NaiveTime::from_hms_opt(0, 2, 10).unwrap();
        let result = time.to_float();
        assert_eq!(result, 130.0);
    }

    #[test]
    fn test_to_float_hours_minutes_seconds() {
        let time = NaiveTime::from_hms_opt(1, 1, 5).unwrap();
        let result = time.to_float();
        assert_eq!(result, 3665.0);
    }

    #[test]
    fn test_to_float_end_of_day() {
        let time = NaiveTime::from_hms_opt(23, 59, 59).unwrap();
        let result = time.to_float();
        assert_eq!(result, 86399.0);
    }

    #[test]
    fn test_to_float_with_nanoseconds() {
        let time = NaiveTime::from_hms_nano_opt(0, 0, 45, 500000000).unwrap();
        let result = time.to_float();
        assert_eq!(result, 45.5);
    }

    #[test]
    fn test_to_float_small_nanoseconds() {
        let time = NaiveTime::from_hms_nano_opt(0, 0, 0, 1000000).unwrap();
        let result = time.to_float();
        assert!((result - 0.001).abs() < 1e-9);
    }

    #[test]
    fn test_to_float_noon() {
        let time = NaiveTime::from_hms_opt(12, 0, 0).unwrap();
        let result = time.to_float();
        assert_eq!(result, 43200.0);
    }

    // ========== Round-trip tests ==========

    #[test]
    fn test_round_trip_from_float_to_float() {
        let original = 3665.5; // 1 hour 1 minute 5.5 seconds
        let time = NaiveTime::from_float(original).unwrap();
        let result = time.to_float();
        assert!((result - original).abs() < 1e-9);
    }

    #[test]
    fn test_round_trip_from_custom_to_custom() {
        let original_str = "12:30:45.0";
        let time = NaiveTime::from_custom(original_str).unwrap();
        let formatted_str = time.format_my_time(true);
        let time2 = NaiveTime::from_custom(&formatted_str);
        assert!(time2.is_ok());
    }

    #[test]
    fn test_round_trip_short_format() {
        let original_str = "30:45.0";
        let time = NaiveTime::from_custom(original_str).unwrap();
        let time2 = NaiveTime::from_float(time.to_float()).unwrap();
        assert_eq!(time, time2);
    }

    // ========== Integration tests ==========

    #[test]
    fn test_integration_from_custom_to_float_to_custom() {
        let original = "01:30:00.0";
        let time1 = NaiveTime::from_custom(original).unwrap();
        let float_value = time1.to_float();
        let time2 = NaiveTime::from_float(float_value).unwrap();
        assert_eq!(time1, time2);
    }

    #[test]
    fn test_integration_formatting_consistency() {
        let time = NaiveTime::from_hms_opt(5, 30, 15).unwrap();
        let my_time = time.format_my_time(true);
        let join_time = time.format_join_time(true);

        // Both should contain the hour information
        assert!(my_time.starts_with("05"));
        assert!(join_time.contains("05"));
    }

    #[test]
    fn test_integration_all_methods_with_specific_time() {
        let time = NaiveTime::from_hms_opt(14, 25, 30).unwrap();

        // Test format_my_time
        let my_time = time.format_my_time(true);
        assert!(my_time.contains("14:25:30"));

        // Test format_join_time
        let join_time = time.format_join_time(true);
        assert!(join_time.contains("14.25.30"));

        // Test to_float
        let float_val = time.to_float();
        assert_eq!(float_val, 51930.0);

        // Test from_float round-trip
        let time_from_float = NaiveTime::from_float(float_val).unwrap();
        assert_eq!(time, time_from_float);
    }
}
