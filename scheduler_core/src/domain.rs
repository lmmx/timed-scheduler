use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintType {
    Before,
    After,
    Apart,
    ApartFrom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintRef {
    WithinGroup,
    Unresolved(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintExpr {
    pub time_hours: u32,
    pub ctype: ConstraintType,
    pub cref: ConstraintRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Frequency {
    Daily,
    TwiceDaily,
    ThreeTimesDaily,
    EveryXHours(u32),
}

impl Frequency {
    pub fn instances_per_day(&self) -> usize {
        match self {
            Self::Daily => 1,
            Self::TwiceDaily => 2,
            Self::ThreeTimesDaily => 3,
            Self::EveryXHours(h) => (24 / *h) as usize,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowSpec {
    Anchor(i32),
    Range(i32, i32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub category: String,
    pub frequency: Frequency,
    pub constraints: Vec<ConstraintExpr>,
    pub windows: Vec<WindowSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduleStrategy {
    Earliest,
    Latest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    pub day_start_minutes: i32,
    pub day_end_minutes: i32,
    pub strategy: ScheduleStrategy,
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
