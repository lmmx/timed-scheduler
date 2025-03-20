use wasm_bindgen::prelude::*;
use scheduler_core::{domain::Entity, solve_schedule};

// Keep the original function, but add better error messages
#[wasm_bindgen]
pub fn schedule_from_json(entities_json: &str) -> String {
    // 1) Deserialize input from JSON → Vec<Entity>
    let entities: Vec<Entity> = match serde_json::from_str(entities_json) {
        Ok(e) => e,
        Err(e) => {
            return format!("Error parsing JSON: {}", e);
        }
    };

    // 2) Call into your scheduler_core solver
    match solve_schedule(&entities) {
        Ok(schedule) => {
            // Convert the schedule (Vec<(String, f64)>) into JSON
            match serde_json::to_string(&schedule) {
                Ok(json) => json,
                Err(e) => format!("Error serializing schedule: {}", e)
            }
        }
        Err(err_str) => {
            // Enhance error messages for common problems
            if err_str.contains("infeasible") || err_str.contains("Infeasible") {
                format!(
                    "Infeasible or error: \
                     There are conflicting constraints in your schedule. \
                     Some tasks may have time windows outside the allowed day range (8:00–18:00), \
                     or the constraints between tasks cannot be satisfied. \
                     Please check your task times and try again."
                )
            } else {
                format!("Infeasible or error: {}", err_str)
            }
        }
    }
}
