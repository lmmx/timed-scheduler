use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Frequency {
    Daily,               // Once daily (aliases: "1x daily", "1x /d", "1x /1d")
    TwiceDaily,          // Twice daily (aliases: "2x daily", "2x /d", "2x /1d")
    ThreeTimesDaily,     // Three times daily (aliases: "3x daily", "3x /d", "3x /1d")
    EveryXHours(u8),     // Every X hours
    Custom(Vec<String>), // For custom time specifications
}

impl Frequency {
    pub fn from_str(freq_str: &str) -> Result<Self, String> {
        // Normalize the string (lowercase, remove extra spaces)
        let freq_str = freq_str.trim().to_lowercase();

        // Regular expressions for matching different formats
        let daily_re = Regex::new(r"^(daily|1x\s*daily|1x\s*/d|1x\s*/1d)$").unwrap();
        let twice_re = Regex::new(r"^(twice\s*daily|2x\s*daily|2x\s*/d|2x\s*/1d)$").unwrap();
        let thrice_re = Regex::new(r"^(thrice\s*daily|3x\s*daily|3x\s*/d|3x\s*/1d)$").unwrap();
        let every_hours_re = Regex::new(r"^every\s*(\d+)\s*hours?$").unwrap();

        if daily_re.is_match(&freq_str) {
            Ok(Frequency::Daily)
        } else if twice_re.is_match(&freq_str) {
            Ok(Frequency::TwiceDaily)
        } else if thrice_re.is_match(&freq_str) {
            Ok(Frequency::ThreeTimesDaily)
        } else if let Some(caps) = every_hours_re.captures(&freq_str) {
            let hours: u8 = caps[1]
                .parse()
                .map_err(|_| "Invalid hour format".to_string())?;
            Ok(Frequency::EveryXHours(hours))
        } else {
            Err(format!("Unrecognized frequency format: {}", freq_str))
        }
    }

    pub fn get_instances_per_day(&self) -> usize {
        match self {
            Frequency::Daily => 1,
            Frequency::TwiceDaily => 2,
            Frequency::ThreeTimesDaily => 3,
            Frequency::EveryXHours(hours) => 24 / *hours as usize,
            Frequency::Custom(times) => times.len(),
        }
    }
}
