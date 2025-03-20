use scheduler_core::domain::{Entity, WindowSpec};
use scheduler_core::solve_schedule;

fn main() {
    // 1) Create some test entities
    let entities = vec![
        Entity {
            name: "Task A".to_string(),
            // Prefers around 09:00 ± 30m
            windows: vec![WindowSpec::Anchor(9 * 60)],
        },
        Entity {
            name: "Lunch".to_string(),
            // Prefers around 12:00 ± 30m
            windows: vec![WindowSpec::Anchor(12 * 60)],
        },
        Entity {
            name: "Task B".to_string(),
            // Must be between 13:00–15:00
            windows: vec![WindowSpec::Range(13 * 60, 15 * 60)],
        },
    ];

    // 2) Solve
    match solve_schedule(&entities) {
        Ok(schedule) => {
            println!("--- Optimized Schedule ---");
            for (name, start) in schedule {
                let hh = (start / 60.0).floor() as i32;
                let mm = (start % 60.0).round() as i32;
                println!("{:02}:{:02} - {}", hh, mm, name);
            }
        }
        Err(e) => eprintln!("Scheduling error: {}", e),
    }
}
