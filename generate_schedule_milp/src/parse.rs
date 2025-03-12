use crate::domain::{Entity, Frequency, ConstraintExpr, ConstraintType, ConstraintRef};
use regex::Regex;

pub fn parse_from_table(rows: Vec<Vec<&str>>) -> Result<Vec<Entity>, String> {
    let re = Regex::new(r#""([^"]+)""#).unwrap();
    let mut out = Vec::new();
    
    for row in rows.into_iter().skip(1) {
        if row.len() < 7 { return Err("Bad row data".to_string()); }
        
        let name = row[0];
        let cat = row[1];
        let freq_str = row[5];
        let cstr = row[6];

        let freq = Frequency::from_str(freq_str);

        let mut cexprs = Vec::new();
        if !cstr.trim().is_empty() && cstr.trim() != "[]" {
            for cap in re.captures_iter(cstr) {
                let txt = cap[1].trim();
                let ce = parse_one_constraint(txt)?;
                cexprs.push(ce);
            }
        }

        out.push(Entity {
            name: name.to_string(),
            category: cat.to_string(),
            frequency: freq,
            constraints: cexprs,
        });
    }
    
    Ok(out)
}

pub fn parse_one_constraint(s: &str) -> Result<ConstraintExpr, String> {
    let re_apart = Regex::new(r"^≥(\d+)h\s+apart$").unwrap();
    let re_before = Regex::new(r"^≥(\d+)h\s+before\s+(.+)$").unwrap();
    let re_after = Regex::new(r"^≥(\d+)h\s+after\s+(.+)$").unwrap();
    let re_afrom = Regex::new(r"^≥(\d+)h\s+apart\s+from\s+(.+)$").unwrap();

    if let Some(cap) = re_apart.captures(s) {
        let hrs: u32 = cap[1].parse().map_err(|_| "Bad hr".to_string())?;
        return Ok(ConstraintExpr {
            time_hours: hrs,
            ctype: ConstraintType::Apart,
            cref: ConstraintRef::WithinGroup,
        });
    }
    
    if let Some(cap) = re_before.captures(s) {
        let hrs: u32 = cap[1].parse().map_err(|_| "Bad hr".to_string())?;
        let rstr = cap[2].trim().to_string();
        return Ok(ConstraintExpr {
            time_hours: hrs,
            ctype: ConstraintType::Before,
            cref: ConstraintRef::Unresolved(rstr),
        });
    }
    
    if let Some(cap) = re_after.captures(s) {
        let hrs: u32 = cap[1].parse().map_err(|_| "Bad hr".to_string())?;
        let rstr = cap[2].trim().to_string();
        return Ok(ConstraintExpr {
            time_hours: hrs,
            ctype: ConstraintType::After,
            cref: ConstraintRef::Unresolved(rstr),
        });
    }
    
    if let Some(cap) = re_afrom.captures(s) {
        let hrs: u32 = cap[1].parse().map_err(|_| "Bad hr".to_string())?;
        let rstr = cap[2].trim().to_string();
        return Ok(ConstraintExpr {
            time_hours: hrs,
            ctype: ConstraintType::ApartFrom,
            cref: ConstraintRef::Unresolved(rstr),
        });
    }
    
    Err(format!("Unknown constraint expr: {}", s))
}