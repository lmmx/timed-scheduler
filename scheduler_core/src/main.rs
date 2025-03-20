use scheduler_core::domain::{Entity, Frequency, ScheduleConfig, ScheduleStrategy, WindowSpec};
use scheduler_core::solve_schedule;

fn main() {
    let config = ScheduleConfig {
        day_start_minutes: 8 * 60, // 08:00
        day_end_minutes: 18 * 60,  // 18:00
        strategy: ScheduleStrategy::Earliest,
        global_windows: vec![],
    };

    let entities = vec![
        Entity {
            name: "Task A".to_string(),
            category: "Work".to_string(),
            frequency: Frequency::Daily,
            constraints: vec![],
            windows: vec![WindowSpec::Anchor(9 * 60)], // Prefers 09:00
        },
        Entity {
            name: "Task B".to_string(),
            category: "Work".to_string(),
            frequency: Frequency::Daily,
            constraints: vec![],
            windows: vec![WindowSpec::Range(13 * 60, 15 * 60)], // 13:00–15:00 window
        },
        Entity {
            name: "Lunch".to_string(),
            category: "Break".to_string(),
            frequency: Frequency::Daily,
            constraints: vec![],
            windows: vec![WindowSpec::Anchor(12 * 60)], // 12:00 preferred
        },
    ];

    match solve_schedule(&entities, &config) {
        Ok(solution) => {
            println!("--- Schedule ---");
            for (name, minutes) in solution {
                let hours = (minutes / 60.0).floor() as u32;
                let mins = (minutes % 60.0).round() as u32;
                println!("{:02}:{:02} - {}", hours, mins, name);
            }
        }
        Err(e) => eprintln!("Error solving schedule: {}", e),
    }
}
