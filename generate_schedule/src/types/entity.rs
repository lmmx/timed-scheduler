use serde::{Deserialize, Serialize};
use crate::types::frequency::Frequency;
use crate::types::constraints::ConstraintExpression;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub category: String,
    pub unit: String,
    pub amount: Option<f64>,
    pub split: Option<i32>,
    pub frequency: Frequency,
    pub constraints: Vec<ConstraintExpression>,
    pub note: Option<String>,
}

impl Entity {
    pub fn new(
        name: &str,
        category: &str,
        unit: &str,
        amount: Option<f64>,
        split: Option<i32>,
        frequency_str: &str,
        constraints: Vec<&str>,
        note: Option<&str>,
    ) -> Result<Self, String> {
        let frequency = Frequency::from_str(frequency_str)?;

        let constraint_expressions = constraints
            .into_iter()
            .map(|s| ConstraintExpression::parse(s))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Entity {
            name: name.to_string(),
            category: category.to_string(),
            unit: unit.to_string(),
            amount,
            split,
            frequency,
            constraints: constraint_expressions,
            note: note.map(|s| s.to_string()),
        })
    }
}