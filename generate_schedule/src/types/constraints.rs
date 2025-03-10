use regex::Regex;
use serde::{Deserialize, Serialize};
use crate::types::time_unit::TimeUnit;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintType {
    Before,    // Target must be scheduled before reference
    After,     // Target must be scheduled after reference
    ApartFrom, // Target must be separated from reference (both before and after)
    Apart,     // Used within recurring instances of the same entity
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintExpression {
    pub time_value: u32,
    pub time_unit: TimeUnit,
    pub constraint_type: ConstraintType,
    pub reference: ConstraintReference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintReference {
    Unresolved(String), // A specific entity by name or all in a category (resolved later)
    WithinGroup,        // For 'apart' constraints within recurring instances
}

impl ConstraintExpression {
    pub fn parse(expr: &str) -> Result<Self, String> {
        // Clean up the input string
        let expr = expr.trim();

        // Regular expressions for different constraint patterns
        let before_re = Regex::new(r"^≥(\d+)([hm])\s+before\s+(.+)$").unwrap();
        let after_re = Regex::new(r"^≥(\d+)([hm])\s+after\s+(.+)$").unwrap();
        let apart_from_re = Regex::new(r"^≥(\d+)([hm])\s+apart\s+from\s+(.+)$").unwrap();
        let apart_re = Regex::new(r"^≥(\d+)([hm])\s+apart$").unwrap();

        if let Some(caps) = before_re.captures(expr) {
            let time_value: u32 = caps[1]
                .parse()
                .map_err(|_| "Invalid time value".to_string())?;
            let time_unit = TimeUnit::from_str(&caps[2])?;
            let reference_str = parse_reference(&caps[3])?;

            Ok(ConstraintExpression {
                time_value,
                time_unit,
                constraint_type: ConstraintType::Before,
                reference: ConstraintReference::Unresolved(reference_str),
            })
        } else if let Some(caps) = after_re.captures(expr) {
            let time_value: u32 = caps[1]
                .parse()
                .map_err(|_| "Invalid time value".to_string())?;
            let time_unit = TimeUnit::from_str(&caps[2])?;
            let reference_str = parse_reference(&caps[3])?;

            Ok(ConstraintExpression {
                time_value,
                time_unit,
                constraint_type: ConstraintType::After,
                reference: ConstraintReference::Unresolved(reference_str),
            })
        } else if let Some(caps) = apart_from_re.captures(expr) {
            let time_value: u32 = caps[1]
                .parse()
                .map_err(|_| "Invalid time value".to_string())?;
            let time_unit = TimeUnit::from_str(&caps[2])?;
            let reference_str = parse_reference(&caps[3])?;

            Ok(ConstraintExpression {
                time_value,
                time_unit,
                constraint_type: ConstraintType::ApartFrom,
                reference: ConstraintReference::Unresolved(reference_str),
            })
        } else if let Some(caps) = apart_re.captures(expr) {
            let time_value: u32 = caps[1]
                .parse()
                .map_err(|_| "Invalid time value".to_string())?;
            let time_unit = TimeUnit::from_str(&caps[2])?;

            Ok(ConstraintExpression {
                time_value,
                time_unit,
                constraint_type: ConstraintType::Apart,
                reference: ConstraintReference::WithinGroup,
            })
        } else {
            Err(format!("Could not parse constraint expression: {}", expr))
        }
    }
}

fn parse_reference(reference: &str) -> Result<String, String> {
    Ok(reference.trim().to_string())
}
