use wasm_bindgen::prelude::*;
use scheduler_core::{solve_schedule, ScheduleConfig, Entity};

#[wasm_bindgen]
pub fn schedule_from_json(entities_json: &str, config_json: &str) -> String {
    let entities: Vec<Entity> = serde_json::from_str(entities_json).unwrap();
    let config: ScheduleConfig = serde_json::from_str(config_json).unwrap();

    match solve_schedule(&entities, &config) {
        Ok(result) => serde_json::to_string(&result).unwrap(),
        Err(e) => format!("Error: {}", e),
    }
}
