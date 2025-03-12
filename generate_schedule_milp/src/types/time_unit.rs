use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TimeUnit {
    Minute,
    Hour,
}

impl TimeUnit {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "m" | "min" | "minute" | "minutes" => Ok(TimeUnit::Minute),
            "h" | "hr" | "hour" | "hours" => Ok(TimeUnit::Hour),
            _ => Err(format!("Unknown time unit: {}", s)),
        }
    }

    pub fn to_minutes(&self, value: u32) -> u32 {
        match self {
            TimeUnit::Minute => value,
            TimeUnit::Hour => value * 60,
        }
    }
}
