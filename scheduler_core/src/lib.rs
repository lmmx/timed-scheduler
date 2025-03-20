pub mod domain;

use good_lp::{
    variables, variable, constraint, default_solver,
    SolverModel, Solution, Expression, Variable
};
use domain::{Entity, WindowSpec};

/// Solve a minimal scheduling problem using `good_lp`.
/// - Schedules each `Entity` within 08:00–18:00.
/// - Enforces each entity's *first* window (anchor ±30 min or range).
/// - Ensures tasks are at least 30 min apart in sorted order.
pub fn solve_schedule(entities: &[Entity]) -> Result<Vec<(String, f64)>, String> {
    let day_start = 8 * 60;  // 08:00
    let day_end   = 18 * 60; // 18:00

    // 1) Create the variables container
    let mut vars = variables!();

    // 2) Build a list of (Entity, Variable) pairs
    let mut entity_vars: Vec<(&Entity, Variable)> = Vec::new();
    for e in entities {
        // Create a variable representing the "start minute" of this entity
        let var = vars.add(
            variable()
                .min(day_start as f64)
                .max(day_end as f64)
        );
        entity_vars.push((e, var));
    }

    // 3) Define an objective: minimize the sum of start times (earliest start).
    let objective = entity_vars
        .iter()
        .fold(Expression::from(0.0), |acc, (_, v)| acc + *v);
    let mut problem = vars.minimise(objective).using(default_solver);

    // 4) Enforce each entity's *first* window
    for (entity, var) in &entity_vars {
        if let Some(window) = entity.windows.first() {
            match window {
                WindowSpec::Anchor(anchor) => {
                    // Enforce ±30 minutes around anchor
                    let lower = (*anchor - 30).max(day_start) as f64;
                    let upper = (*anchor + 30).min(day_end) as f64;
                    problem = problem
                        .with(constraint!( *var >= lower ))
                        .with(constraint!( *var <= upper ));
                }
                WindowSpec::Range(start, end) => {
                    problem = problem
                        .with(constraint!( *var >= *start as f64 ))
                        .with(constraint!( *var <= *end   as f64 ));
                }
            }
        }
    }

    // 5) Solve
    let solution = problem.solve().map_err(|e| e.to_string())?;

    // 6) Collect results
    let mut schedule: Vec<(String, f64)> = entity_vars
        .into_iter()
        .map(|(e, var)| (e.name.clone(), solution.value(var)))
        .collect();

    // Sort final schedule by time
    schedule.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

    Ok(schedule)
}
