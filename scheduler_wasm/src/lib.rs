use scheduler_core::{domain::Entity, solve_schedule};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

/// Configuration for the scheduling problem.
/// This structure allows for dynamic specification of scheduling parameters
/// including the tasks to be scheduled and optional day boundaries.
#[derive(Debug, Deserialize)]
struct ScheduleConfig {
    /// The list of tasks/entities to be scheduled
    tasks: Vec<Entity>,

    /// Optional day start time in minutes (e.g., 480 for 08:00)
    #[serde(rename = "dayStart")]
    day_start: Option<i32>,

    /// Optional day end time in minutes (e.g., 1080 for 18:00)
    #[serde(rename = "dayEnd")]
    day_end: Option<i32>,
}

#[wasm_bindgen]
pub fn schedule_from_json(entities_json: &str) -> String {
    // Parse input as a config object
    let config: ScheduleConfig = match serde_json::from_str(entities_json) {
        Ok(c) => c,
        Err(e) => {
            // Try to parse as just an array of entities for backward compatibility
            let entities: Vec<Entity> = match serde_json::from_str(entities_json) {
                Ok(e) => e,
                Err(_) => {
                    return format!("Error parsing JSON: {}", e);
                }
            };
            ScheduleConfig {
                tasks: entities,
                day_start: None,
                day_end: None,
            }
        }
    };

    // Call into the scheduler_core solver with the day parameters
    match solve_schedule(&config.tasks, config.day_start, config.day_end) {
        Ok(schedule) => {
            // Convert the schedule (Vec<(String, f64)>) into JSON
            match serde_json::to_string(&schedule) {
                Ok(json) => json,
                Err(e) => format!("Error serializing schedule: {}", e),
            }
        }
        Err(err_str) => format!("Infeasible or error: {}", err_str),
    }
}
