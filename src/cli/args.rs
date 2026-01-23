use anyhow::Result;

#[derive(clap::Args)]
pub struct OptimizeArgs {
    /// Min quality for image optimization (0-100)
    #[clap(long, visible_alias = "mq", default_value_t = 0, value_parser = quality_parser)]
    pub min_quality: u8,

    /// Max quality for image optimization (0-100)
    #[clap(long, visible_alias = "Mq", default_value_t = 30, value_parser = quality_parser)]
    pub max_quality: u8,

    /// Speed for image optimization (1-10)
    #[clap(long, visible_alias = "sp", default_value_t = 8, value_parser = speed_parser)]
    pub speed: i32,

    /// Minimum file size (in KB) to optimize
    #[clap(long, visible_alias = "mk", default_value_t = 50)]
    pub min_size_kb: u64,
}

fn quality_parser(s: &str) -> Result<u8, String> {
    s.parse::<u8>()
        .map_err(|_| {
            format!(
                "'{}' is not a valid number. Quality must be an integer between 0 and 100.",
                s
            )
        })
        .and_then(|value| {
            if value <= 100 {
                Ok(value)
            } else {
                Err(format!("Quality must be between 0 and 100, got {}", value))
            }
        })
}

fn speed_parser(s: &str) -> Result<i32, String> {
    s.parse::<i32>()
        .map_err(|_| {
            format!(
                "'{}' is not a valid number. Speed must be an integer between 1 and 10.",
                s
            )
        })
        .and_then(|value| {
            if (1..=10).contains(&value) {
                Ok(value)
            } else {
                Err(format!("Speed must be between 1 and 10, got {}", value))
            }
        })
}
