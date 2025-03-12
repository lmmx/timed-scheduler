use std::env;

#[derive(Debug, Clone, Copy)]
pub enum ScheduleStrategy {
    Earliest,
    Latest,
}

#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    pub day_start_minutes: i32,  // e.g. 8*60 for 8:00 AM
    pub day_end_minutes: i32,    // e.g. 22*60 for 10:00 PM
    pub strategy: ScheduleStrategy,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            day_start_minutes: 8 * 60,  // 8:00 AM default
            day_end_minutes: 22 * 60,   // 10:00 PM default
            strategy: ScheduleStrategy::Earliest,
        }
    }
}

/// Parses command-line arguments to set:
/// - day window start/end via --start=HH:MM and --end=HH:MM
/// - scheduling strategy (earliest vs. latest), e.g. "cargo run latest"
pub fn parse_config_from_args() -> ScheduleConfig {
    let args: Vec<String> = env::args().collect();
    let mut config = ScheduleConfig::default();

    // 1) Day window
    if let Some(start_arg) = args.iter().find(|a| a.starts_with("--start=")) {
        if let Some(time_str) = start_arg.strip_prefix("--start=") {
            if let Some((h_str, m_str)) = time_str.split_once(':') {
                if let (Ok(h), Ok(m)) = (h_str.parse::<i32>(), m_str.parse::<i32>()) {
                    config.day_start_minutes = h * 60 + m;
                }
            }
        }
    }

    if let Some(end_arg) = args.iter().find(|a| a.starts_with("--end=")) {
        if let Some(time_str) = end_arg.strip_prefix("--end=") {
            if let Some((h_str, m_str)) = time_str.split_once(':') {
                if let (Ok(h), Ok(m)) = (h_str.parse::<i32>(), m_str.parse::<i32>()) {
                    config.day_end_minutes = h * 60 + m;
                }
            }
        }
    }

    // 2) Strategy: if user typed "latest" anywhere, we switch from Earliest to Latest
    // e.g. "cargo run latest" or "cargo run -- latest"
    if args.iter().any(|a| a.eq_ignore_ascii_case("latest")) {
        config.strategy = ScheduleStrategy::Latest;
    }

    // Otherwise it remains Earliest (the default)
    config
}