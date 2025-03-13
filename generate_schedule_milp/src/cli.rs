use std::env;
use crate::domain::WindowSpec;

#[derive(Debug, Clone, Copy)]
pub enum ScheduleStrategy {
    Earliest,
    Latest,
}

#[derive(Debug, Clone)]
pub struct ScheduleConfig {
    pub day_start_minutes: i32,  // e.g. 8*60
    pub day_end_minutes: i32,    // e.g. 22*60
    pub strategy: ScheduleStrategy,

    // New field for global windows
    pub global_windows: Vec<WindowSpec>,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            day_start_minutes: 8 * 60,
            day_end_minutes: 22 * 60,
            strategy: ScheduleStrategy::Earliest,
            global_windows: Vec::new(),
        }
    }
}

pub fn parse_config_from_args() -> ScheduleConfig {
    let args: Vec<String> = env::args().collect();
    let mut config = ScheduleConfig::default();

    // 1) Day window
    let parse_time_arg = |prefix: &str, minutes: &mut i32| {
        args.iter()
            .find_map(|arg| arg.strip_prefix(prefix))
            .and_then(|time_str| time_str.split_once(':'))
            .and_then(|(h_str, m_str)| {
                h_str.parse::<i32>().ok().zip(m_str.parse::<i32>().ok())
                    .map(|(h, m)| *minutes = h * 60 + m)
            });
    };
    parse_time_arg("--start=", &mut config.day_start_minutes);
    parse_time_arg("--end=", &mut config.day_end_minutes);

    // 2) Strategy
    if args.iter().any(|a| a.eq_ignore_ascii_case("latest")) {
        config.strategy = ScheduleStrategy::Latest;
    }

    // 3) Global windows: e.g. --windows=08:00,12:00-13:00,18:00
    // We parse them similarly to parse_one_window in parse.rs but inlined for brevity.
    if let Some(win_arg) = args.iter().find(|a| a.starts_with("--windows=")) {
        let raw = &win_arg["--windows=".len()..];
        config.global_windows = parse_windows_string(raw)
            .unwrap_or_else(|e| {
                eprintln!("Warning: could not parse windows from '{}': {}", raw, e);
                Vec::new()
            });
    }

    config
}

// Minimal parse logic for CLI windows.
// This can mirror your parse_one_window from parse.rs, or call a shared function.
fn parse_windows_string(input: &str) -> Result<Vec<WindowSpec>, String> {
    // Example: "08:00,12:00-13:00,19:00"
    let parts: Vec<_> = input.split(',').map(|p| p.trim()).collect();
    let mut specs = Vec::new();
    for part in parts {
        if part.is_empty() {
            continue;
        }
        if let Some(idx) = part.find('-') {
            // Range
            let (start_str, end_str) = part.split_at(idx);
            let end_str = &end_str[1..];
            let start_min = hhmm_to_minutes(start_str.trim())?;
            let end_min = hhmm_to_minutes(end_str.trim())?;
            if end_min < start_min {
                return Err(format!("Invalid window range: {}", part));
            }
            specs.push(WindowSpec::Range(start_min, end_min));
        } else {
            // Anchor
            let anchor = hhmm_to_minutes(part)?;
            specs.push(WindowSpec::Anchor(anchor));
        }
    }
    Ok(specs)
}

// Simplified version of parse_hhmm_to_minutes
fn hhmm_to_minutes(hhmm: &str) -> Result<i32, String> {
    let mut split = hhmm.split(':');
    let h = split.next().ok_or("Missing hour")?.parse::<i32>().map_err(|_| "Bad hour")?;
    let m = split.next().ok_or("Missing minute")?.parse::<i32>().map_err(|_| "Bad minute")?;
    if !(0..=23).contains(&h) || !(0..=59).contains(&m) {
        return Err(format!("Out of range: {}", hhmm));
    }
    Ok(h * 60 + m)
}
