use crate::compiler::debugging::debug_print;
use crate::compiler::time_constraint_compiler::TimeConstraintCompiler;
use clock_zones::Constraint;

pub fn apply_daily_bounds(compiler: &mut TimeConstraintCompiler) -> Result<(), String> {
    // Convert time to minutes (0-1440 for a 24-hour day)
    for (clock_id, clock_info) in &compiler.clocks {
        // Not before 0:00
        compiler
            .zone
            .add_constraint(Constraint::new_ge(clock_info.variable, 0));
        // Not after 23:59
        compiler
            .zone
            .add_constraint(Constraint::new_le(clock_info.variable, 1440));

        if compiler.debug {
            debug_print(
                compiler,
                "⏱️",
                &format!("Set bounds for {}: [0:00, 23:59]", clock_id),
            );
        }
    }

    Ok(())
}
