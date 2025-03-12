use crate::compiler::time_constraint_compiler::TimeConstraintCompiler;
use clock_zones::Zone;
use std::collections::HashMap;

// Extract a concrete schedule from the zone
pub fn extract_schedule(compiler: &TimeConstraintCompiler) -> Result<HashMap<String, i32>, String> {
    if compiler.zone.is_empty() {
        return Err("Cannot extract schedule from empty zone".to_string());
    }

    let mut schedule = HashMap::new();

    // For each clock, get a feasible time
    for (clock_id, clock_info) in &compiler.clocks {
        // Get the lower and upper bounds for this clock (in minutes)
        let lower = compiler
            .zone
            .get_lower_bound(clock_info.variable)
            .unwrap_or(0);
        let upper = compiler
            .zone
            .get_upper_bound(clock_info.variable)
            .unwrap_or(1440);

        // Choose a time in the middle of the feasible range
        let time_in_minutes = ((lower + upper) / 2) as i32;
        schedule.insert(clock_id.clone(), time_in_minutes);
    }

    Ok(schedule)
}

// Format the schedule into a human-readable format
pub fn format_schedule(
    compiler: &TimeConstraintCompiler,
    schedule: &HashMap<String, i32>,
) -> String {
    let mut result = String::new();
    result.push_str("Daily Schedule:\n");

    // Convert minutes to HH:MM format and sort by time
    let mut time_entries: Vec<(String, String)> = schedule
        .iter()
        .map(|(clock_id, &minutes)| {
            let hours = minutes / 60;
            let mins = minutes % 60;
            let time_str = format!("{:02}:{:02}", hours, mins);
            (time_str.clone(), format!("{}: {}", clock_id, time_str))
        })
        .collect();

    // Sort by time
    time_entries.sort_by(|a, b| a.0.cmp(&b.0));

    // Add to result
    for (_, entry) in time_entries {
        result.push_str(&format!("  {}\n", entry));
    }

    // Group by entity
    result.push_str("\nBy Entity:\n");

    let mut entity_schedules: HashMap<String, Vec<(String, i32)>> = HashMap::new();

    for (clock_id, &minutes) in schedule {
        if let Some(clock_info) = compiler.clocks.get(clock_id) {
            entity_schedules
                .entry(clock_info.entity_name.clone())
                .or_default()
                .push((clock_id.clone(), minutes));
        }
    }

    // Sort entities alphabetically
    let mut entity_names: Vec<String> = entity_schedules.keys().cloned().collect();
    entity_names.sort();

    for entity_name in entity_names {
        let entity = compiler.entities.get(&entity_name).unwrap();
        result.push_str(&format!("  {} ({}):\n", entity_name, entity.category));

        let times = entity_schedules.get(&entity_name).unwrap();
        let mut sorted_times = times.clone();
        sorted_times.sort_by_key(|&(_, minutes)| minutes);

        for (clock_id, minutes) in sorted_times {
            let hours = minutes / 60;
            let mins = minutes % 60;
            result.push_str(&format!("    {}: {:02}:{:02}", clock_id, hours, mins));

            // Add amount information if available
            if let Some(entity) = compiler.entities.get(&entity_name) {
                if let Some(amount) = entity.amount {
                    if let Some(split) = entity.split {
                        // If we have both amount and split
                        let per_instance = amount / split as f64;
                        result.push_str(&format!(" - {:.1} {}", per_instance, entity.unit));
                    } else {
                        // If we have just amount
                        result.push_str(&format!(" - {:.1} {}", amount, entity.unit));
                    }
                } else if let Some(split) = entity.split {
                    // If we have just split
                    result.push_str(&format!(" - 1/{} {}", split, entity.unit));
                }
            }

            result.push_str("\n");
        }
    }

    result
}
