use crate::domain::{
    Entity, Frequency, ConstraintExpr, ConstraintType, ConstraintRef,
    WindowSpec, // newly introduced in domain.rs
};
use regex::Regex;

/// Parse the table into a list of `Entity`.
/// Now expects a table with at least 9 columns:
///   [0]: Entity
///   [1]: Category
///   [2]: Unit
///   [3]: Amount
///   [4]: Split
///   [5]: Frequency
///   [6]: Constraints
///   [7]: Windows   (new)
///   [8]: Note
///
/// Returns an error if rows have fewer than 9 columns.
pub fn parse_from_table(rows: Vec<Vec<&str>>) -> Result<Vec<Entity>, String> {
    // For parsing the constraints text (JSON-like array of strings),
    // we reuse the same approach with a regex capturing anything in quotes.
    let re = Regex::new(r#""([^"]+)""#).unwrap();

    rows.into_iter()
        .skip(1) // skip header row
        .map(|row| {
            // we now expect at least 9 columns
            if row.len() < 9 {
                return Err(format!(
                    "Bad row data: expected at least 9 columns, got {}",
                    row.len()
                ));
            }

            // (1) parse constraints
            let constraints_str = row[6].trim();
            let cexprs = match constraints_str {
                "" | "[]" => Vec::new(),
                _ => re
                    .captures_iter(constraints_str)
                    .map(|cap| parse_one_constraint(cap[1].trim()))
                    .collect::<Result<Vec<_>, _>>()?,
            };

            // (2) parse windows
            let windows_str = row[7].trim();
            let wspecs = match windows_str {
                "" | "[]" => Vec::new(),
                _ => {
                    re.captures_iter(windows_str)
                        .map(|cap| parse_one_window(cap[1].trim()))
                        .collect::<Result<Vec<_>, _>>()?
                }
            };

            // (3) build the entity
            Ok(Entity {
                name: row[0].to_string(),
                category: row[1].to_string(),
                frequency: Frequency::from_str(row[5]),
                constraints: cexprs,
                windows: wspecs, // new field in Entity
            })
        })
        .collect()
}

/// Parse a single constraint snippet, e.g. "≥8h apart", "≥1h before food", etc.
///
/// For example, the string "≥6h apart" is recognized as:
///   - time_hours = 6
///   - ctype = ConstraintType::Apart
///   - cref = ConstraintRef::WithinGroup (since "apart" was recognized)
pub fn parse_one_constraint(s: &str) -> Result<ConstraintExpr, String> {
    let patterns = &[
        (r"^≥(\d+)h\s+apart$",              ConstraintType::Apart,     true),
        (r"^≥(\d+)h\s+before\s+(.+)$",      ConstraintType::Before,    false),
        (r"^≥(\d+)h\s+after\s+(.+)$",       ConstraintType::After,     false),
        (r"^≥(\d+)h\s+apart\s+from\s+(.+)$",ConstraintType::ApartFrom, false),
    ];

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
                Ok(ConstraintExpr {
                    time_hours: hrs,
                    ctype: ctype.clone(),
                    cref,
                })
            })
        })
        .unwrap_or_else(|| Err(format!("Unknown constraint expr: {}", s)))
}

/// Parse a single window snippet, e.g. "08:00" or "12:00-13:00".
/// Returns a `WindowSpec::Anchor(...)` or `WindowSpec::Range(...)`.
fn parse_one_window(s: &str) -> Result<WindowSpec, String> {
    // If there's a dash, assume "start-end" range
    if let Some(idx) = s.find('-') {
        let (start_str, end_str) = s.split_at(idx);
        // split_at keeps the '-' in the second piece, so skip 1
        let end_str = &end_str[1..];

        let start_min = parse_hhmm_to_minutes(start_str.trim())?;
        let end_min   = parse_hhmm_to_minutes(end_str.trim())?;
        if end_min < start_min {
            return Err(format!(
                "Window range is reversed or invalid: {}",
                s
            ));
        }
        Ok(WindowSpec::Range(start_min, end_min))
    } else {
        // No dash => interpret as an anchor
        let anchor = parse_hhmm_to_minutes(s)?;
        Ok(WindowSpec::Anchor(anchor))
    }
}

/// Convert "HH:MM" to minutes from midnight (0..1440).
fn parse_hhmm_to_minutes(hhmm: &str) -> Result<i32, String> {
    let parts: Vec<_> = hhmm.split(':').collect();
    if parts.len() != 2 {
        return Err(format!("Not in HH:MM format: {}", hhmm));
    }
    let hour: i32 = parts[0].parse().map_err(|_| format!("Bad hour: {}", parts[0]))?;
    let min:  i32 = parts[1].parse().map_err(|_| format!("Bad minute: {}", parts[1]))?;

    if !(0..=23).contains(&hour) || !(0..=59).contains(&min) {
        return Err(format!("Time out of valid range: {}", hhmm));
    }

    Ok(hour * 60 + min)
}
