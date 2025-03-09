use regex::Regex;
use crate::types::entity::Entity;

// Function to parse from the tabular format shown in the example
pub fn parse_from_table(rows: Vec<Vec<&str>>) -> Result<Vec<Entity>, String> {
    let mut entities = Vec::new();

    // Skip header row
    for row in rows.iter().skip(1) {
        if row.len() < 7 {
            return Err("Row has insufficient columns".to_string());
        }

        let name = row[0];
        let category = row[1];
        let unit = row[2];

        // Parse amount (float or null)
        let amount = match row[3] {
            "null" => None,
            s => Some(
                s.parse::<f64>()
                    .map_err(|_| "Invalid amount format".to_string())?,
            ),
        };

        // Parse split (int or null)
        let split = match row[4] {
            "null" => None,
            s => Some(
                s.parse::<i32>()
                    .map_err(|_| "Invalid split format".to_string())?,
            ),
        };

        let frequency = row[5];

        // Parse constraints array (from string to vec)
        let constraints_str = row[6].trim();
        let constraints = if constraints_str == "[]" {
            Vec::new()
        } else {
            // Extract strings between quotes inside the array
            let re = Regex::new(r#""([^"]+)""#).unwrap();
            re.captures_iter(constraints_str)
                .map(|cap| cap[1].to_string())
                .collect::<Vec<String>>()
        };

        // Parse note (optional field, default to None if not present)
        let note = if row.len() > 7 {
            match row[7] {
                "null" => None,
                s => Some(s),
            }
        } else {
            None
        };

        entities.push(Entity::new(
            name,
            category,
            unit,
            amount,
            split,
            frequency,
            constraints.iter().map(|s| s.as_str()).collect(),
            note,
        )?);
    }

    Ok(entities)
}