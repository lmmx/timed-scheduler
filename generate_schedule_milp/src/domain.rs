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
        match () {
            _ if lower.contains("3x") => Self::ThreeTimesDaily,
            _ if lower.contains("2x") => Self::TwiceDaily,
            _ if lower.contains("1x") => Self::Daily,
            _ => Self::EveryXHours(8),
        }
    }

    pub fn instances_per_day(&self) -> usize {
        match self {
            Self::Daily => 1,
            Self::TwiceDaily => 2,
            Self::ThreeTimesDaily => 3,
            Self::EveryXHours(h) => 24 / (*h as usize),
        }
    }
}

/// Represents a desired scheduling “window,” which can be:
///   - A single anchor time (in minutes from midnight), e.g. 480 for 08:00
///   - A start–end range in minutes (e.g. 720..780 for 12:00–13:00)
#[derive(Debug, Clone)]
pub enum WindowSpec {
    Anchor(i32),
    Range(i32, i32),
}

/// An “entity” to be scheduled.
/// - `constraints` are the typical “Apart”, “Before”, etc. constraints
/// - `windows` is optional extra data: if nonempty, the solver may need
///   to place this entity in one of these windows, or near these anchors.
#[derive(Debug, Clone)]
pub struct Entity {
    pub name: String,
    pub category: String,
    pub frequency: Frequency,
    pub constraints: Vec<ConstraintExpr>,

    /// New field: a list of windows (anchors or ranges) associated with this entity.
    /// If empty, the entity has no special windows and may be placed by global logic.
    pub windows: Vec<WindowSpec>,
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
