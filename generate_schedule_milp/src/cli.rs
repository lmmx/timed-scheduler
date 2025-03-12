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
    let parse_time_arg = |prefix: &str, minutes: &mut i32| {
        args.iter()
            .find_map(|arg| arg.strip_prefix(prefix))
            .and_then(|time_str| time_str.split_once(':'))
            .and_then(|(h_str, m_str)| 
                h_str.parse::<i32>().ok().zip(m_str.parse::<i32>().ok())
                    .map(|(h, m)| *minutes = h * 60 + m)
            );
    };

    parse_time_arg("--start=", &mut config.day_start_minutes);
    parse_time_arg("--end=", &mut config.day_end_minutes);

    // 2) Strategy: if user typed "latest" anywhere, we switch from Earliest to Latest
    // e.g. "cargo run latest" or "cargo run -- latest"
    if args.iter().any(|a| a.eq_ignore_ascii_case("latest")) {
        config.strategy = ScheduleStrategy::Latest;
    }

    // Otherwise it remains Earliest (the default)
    config
}
