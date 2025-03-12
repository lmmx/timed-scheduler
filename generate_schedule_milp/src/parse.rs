use crate::domain::{Entity, Frequency, ConstraintExpr, ConstraintType, ConstraintRef};
use regex::Regex;

pub fn parse_from_table(rows: Vec<Vec<&str>>) -> Result<Vec<Entity>, String> {
    let re = Regex::new(r#""([^"]+)""#).unwrap();
    
    rows.into_iter()
        .skip(1) // skip header
        .map(|row| {
            if row.len() < 7 {
                return Err("Bad row data".to_string());
            }
            let cstr = row[6].trim();
            let cexprs = match cstr {
                "" | "[]" => Vec::new(),
                _ => re
                    .captures_iter(cstr)
                    .map(|cap| parse_one_constraint(cap[1].trim()))
                    .collect::<Result<Vec<_>, _>>()?,
            };
            Ok(Entity {
                name: row[0].to_string(),
                category: row[1].to_string(),
                frequency: Frequency::from_str(row[5]),
                constraints: cexprs,
            })
        })
        .collect()
}

pub fn parse_one_constraint(s: &str) -> Result<ConstraintExpr, String> {
    // pattern array: (regex, constraint type, is_within_group)
    let patterns = &[
        (r"^≥(\d+)h\s+apart$",            ConstraintType::Apart,     true),
        (r"^≥(\d+)h\s+before\s+(.+)$",    ConstraintType::Before,    false),
        (r"^≥(\d+)h\s+after\s+(.+)$",     ConstraintType::After,     false),
        (r"^≥(\d+)h\s+apart\s+from\s+(.+)$", ConstraintType::ApartFrom, false),
    ];

    // Use find_map() to locate the first matching pattern and build the ConstraintExpr
    patterns
        .iter()
        .find_map(|(pattern, ctype, is_within_group)| {
            Regex::new(pattern).unwrap().captures(s).map(|cap| {
                let hrs: u32 = cap[1].parse().map_err(|_| "Bad hr".to_string())?;
                let cref = if *is_within_group {
                    ConstraintRef::WithinGroup
                } else {
                    ConstraintRef::Unresolved(cap[2].trim().to_string())
                };
                Ok(ConstraintExpr { time_hours: hrs, ctype: ctype.clone(), cref })
            })
        })
        .unwrap_or_else(|| Err(format!("Unknown constraint expr: {}", s)))
}
