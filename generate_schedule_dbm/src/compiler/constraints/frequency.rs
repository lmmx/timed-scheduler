use crate::compiler::debugging::debug_print;
use crate::compiler::time_constraint_compiler::TimeConstraintCompiler;
use clock_zones::{Constraint, Variable, Zone};
use std::collections::HashMap;

pub fn apply_frequency_constraints(compiler: &mut TimeConstraintCompiler) -> Result<(), String> {
    // Group clocks by entity
    let mut entity_clocks: HashMap<String, Vec<Variable>> = HashMap::new();

    for clock_info in compiler.clocks.values() {
        entity_clocks
            .entry(clock_info.entity_name.clone())
            .or_default()
            .push(clock_info.variable);
    }

    // For each entity, ensure instance ordering and apply default spacing
    for (entity_name, clocks) in entity_clocks {
        if clocks.len() <= 1 {
            continue; // No constraints needed for single instances
        }

        let entity = compiler.entities.get(&entity_name).unwrap();

        // Sort clocks by instance number
        let mut ordered_clocks: Vec<(usize, Variable)> = compiler
            .clocks
            .values()
            .filter(|c| c.entity_name == entity_name)
            .map(|c| (c.instance, c.variable))
            .collect();
        ordered_clocks.sort_by_key(|&(instance, _)| instance);

        // Apply ordering and spacing constraints
        for i in 0..ordered_clocks.len() - 1 {
            let (instance_i, current) = ordered_clocks[i];
            let (instance_j, next) = ordered_clocks[i + 1];

            // Next instance must come after current instance
            compiler
                .zone
                .add_constraint(Constraint::new_diff_gt(next, current, 0));

            if compiler.debug {
                debug_print(
                    compiler,
                    "🔢",
                    &format!(
                        "{}_{}  must be after  {}_{}",
                        entity_name, instance_j, entity_name, instance_i
                    ),
                );
            }

            // Apply minimum spacing only if specified
            let min_spacing = if let Some(spacing) = entity.min_spacing {
                spacing as i64
            } else {
                0 // no default enforced spacing
            };

            compiler
                .zone
                .add_constraint(Constraint::new_diff_ge(next, current, min_spacing));

            if compiler.debug {
                let hours = min_spacing / 60;
                let mins = min_spacing % 60;
                debug_print(
                    compiler,
                    "↔️",
                    &format!(
                        "{}_{}  must be ≥{}h{}m after  {}_{}",
                        entity_name, instance_j, hours, mins, entity_name, instance_i
                    ),
                );
            }
        }
    }

    Ok(())
}
