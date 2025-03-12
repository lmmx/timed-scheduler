use good_lp::variable::Variable;

#[derive(Debug, Clone)]
pub enum ConstraintType {
    Before,
    After,
    Apart,
    ApartFrom,
}

#[derive(Debug, Clone)]
pub enum ConstraintRef {
    WithinGroup,
    Unresolved(String),
}

#[derive(Debug, Clone)]
pub struct ConstraintExpr {
    pub time_hours: u32,
    pub ctype: ConstraintType,
    pub cref: ConstraintRef,
}

#[derive(Debug, Clone)]
pub enum Frequency {
    Daily,
    TwiceDaily,
    ThreeTimesDaily,
    EveryXHours(u32),
}

impl Frequency {
    pub fn from_str(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.contains("3x") {
            Frequency::ThreeTimesDaily
        } else if lower.contains("2x") {
            Frequency::TwiceDaily
        } else if lower.contains("1x") {
            Frequency::Daily
        } else {
            Frequency::EveryXHours(8)
        }
    }
    
    pub fn instances_per_day(&self) -> usize {
        match self {
            Frequency::Daily => 1,
            Frequency::TwiceDaily => 2,
            Frequency::ThreeTimesDaily => 3,
            Frequency::EveryXHours(h) => 24 / (*h as usize),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub name: String,
    pub category: String,
    pub frequency: Frequency,
    pub constraints: Vec<ConstraintExpr>,
}

#[derive(Clone)]
pub struct ClockVar {
    pub entity_name: String,
    pub instance: usize,
    pub var: Variable,
}

pub fn c2str(c: &ClockVar) -> String {
    format!("({}_var{})", c.entity_name, c.instance)
}