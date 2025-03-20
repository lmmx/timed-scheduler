#[derive(Debug, Clone)]
pub enum WindowSpec {
    Anchor(i32),       // e.g. an anchor time like 09:00 (540)
    Range(i32, i32),   // e.g. a range [13:00..15:00]
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub name: String,
    pub windows: Vec<WindowSpec>,
}
