pub mod domain;
use domain::{Entity, ScheduleConfig, WindowSpec};
use good_lp::{default_solver, variable, variables, Solution, SolverModel};

pub fn solve_schedule(
    entities: &[Entity],
    config: &ScheduleConfig,
) -> Result<Vec<(String, f64)>, String> {
    let mut schedule = Vec::new();

    for entity in entities {
        // For simplicity, just pick the earliest window or default start time
        let scheduled_time = if !entity.windows.is_empty() {
            match &entity.windows[0] {
                WindowSpec::Anchor(t) => *t as f64,
                WindowSpec::Range(start, _) => *start as f64,
            }
        } else {
            config.day_start_minutes as f64
        };

        schedule.push((entity.name.clone(), scheduled_time));
    }

    // Sort schedule by time
    schedule.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    Ok(schedule)
}
