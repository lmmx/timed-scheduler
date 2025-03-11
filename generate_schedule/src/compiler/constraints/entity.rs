use crate::compiler::clock_info::ClockInfo;
use crate::compiler::debugging::{debug_error, debug_print};
use crate::compiler::reference_resolution::resolve_reference;
use crate::compiler::time_constraint_compiler::TimeConstraintCompiler;
use crate::types::constraints::{ConstraintExpression, ConstraintReference, ConstraintType};
use crate::types::time_unit::TimeUnit::Hour;
use clock_zones::{Constraint, Variable};

pub fn apply_entity_constraints(compiler: &mut TimeConstraintCompiler) -> Result<(), String> {
    // First, collect all constraint operations we need to perform
    let mut constraint_operations = Vec::new();

    for (entity_name, entity) in &compiler.entities {
        let entity_clocks: Vec<&ClockInfo> = compiler
            .clocks
            .values()
            .filter(|c| c.entity_name == *entity_name)
            .collect();

        // Process all constraint types for this entity
        for constraint in &entity.constraints {
            match &constraint.constraint_type {
                ConstraintType::Apart => {
                    // Existing Apart handling...
                    if entity_clocks.len() <= 1 {
                        continue; // Skip entities with only one instance
                    }

                    // Sort clocks by instance number
                    let mut ordered_clocks = entity_clocks.clone();
                    ordered_clocks.sort_by_key(|c| c.instance);

                    let time_in_minutes =
                        constraint.time_unit.to_minutes(constraint.time_value) as i64;

                    // Create sequential constraints
                    for i in 0..ordered_clocks.len() - 1 {
                        let current = ordered_clocks[i];
                        let next = ordered_clocks[i + 1];

                        // Store the constraint operation for later execution
                        constraint_operations.push((
                            current.variable,
                            next.variable,
                            time_in_minutes,
                            format!(
                                "{} must be ≥{}h{}m after {}",
                                compiler.find_clock_name(next.variable).unwrap_or_default(),
                                time_in_minutes / 60,
                                time_in_minutes % 60,
                                compiler
                                    .find_clock_name(current.variable)
                                    .unwrap_or_default()
                            ),
                        ));
                    }
                }
                ConstraintType::Before | ConstraintType::After => {
                    // Extract reference string first
                    let reference_str = match &constraint.reference {
                        ConstraintReference::Unresolved(ref_str) => ref_str.clone(),
                        ConstraintReference::WithinGroup => "within group".to_string(),
                    };

                    // Get reference clocks based on the constraint reference
                    let _reference_clocks = match &constraint.reference {
                        ConstraintReference::Unresolved(ref_str) => {
                            match resolve_reference(compiler, ref_str) {
                                Ok(clocks) => clocks,
                                Err(e) => {
                                    debug_error(
                                        compiler,
                                        "⚠️",
                                        &format!(
                                            "Could not resolve reference '{}': {}",
                                            ref_str, e
                                        ),
                                    );
                                    continue;
                                }
                            }
                        }
                        ConstraintReference::WithinGroup => {
                            debug_error(
                                compiler,
                                "⚠️",
                                "WithinGroup reference should not be used here",
                            );
                            continue;
                        }
                    };

                    let _time_in_minutes =
                        constraint.time_unit.to_minutes(constraint.time_value) as i64;

                    // Just log that we're handling these constraints
                    if compiler.debug {
                        debug_print(
                            compiler,
                            "ℹ️",
                            &format!(
                                "Special constraint: {} must be ≥{}{}m {} {} (when applicable)",
                                entity_name,
                                constraint.time_value,
                                if constraint.time_unit == Hour {
                                    "h"
                                } else {
                                    "m"
                                },
                                match constraint.constraint_type {
                                    ConstraintType::Before => "before",
                                    ConstraintType::After => "after",
                                    _ => "related to",
                                },
                                reference_str
                            ),
                        );
                    }
                }
                ConstraintType::ApartFrom => {
                    // Handle ApartFrom constraints - these are simpler than Before/After
                    // as they enforce minimum separation regardless of order
                    let reference_clocks = match &constraint.reference {
                        ConstraintReference::Unresolved(reference_str) => {
                            match resolve_reference(compiler, reference_str) {
                                Ok(clocks) => clocks,
                                Err(e) => {
                                    debug_error(
                                        compiler,
                                        "⚠️",
                                        &format!(
                                            "Could not resolve reference '{}': {}",
                                            reference_str, e
                                        ),
                                    );
                                    continue;
                                }
                            }
                        }
                        ConstraintReference::WithinGroup => {
                            debug_error(
                                compiler,
                                "⚠️",
                                "WithinGroup reference should not be used here",
                            );
                            continue;
                        }
                    };

                    let time_in_minutes =
                        constraint.time_unit.to_minutes(constraint.time_value) as i64;

                    // For "apart from", we can add constraints to ensure there's
                    // at least the specified separation in either direction
                    // This creates a "forbidden zone" around each reference clock

                    for entity_var in entity_clocks.iter().map(|c| c.variable) {
                        for &reference_var in &reference_clocks {
                            // Get clock names for better logs
                            let entity_name =
                                compiler.find_clock_name(entity_var).unwrap_or_default();
                            let ref_name =
                                compiler.find_clock_name(reference_var).unwrap_or_default();

                            if compiler.debug {
                                debug_print(
                                    compiler,
                                    "ℹ️",
                                    &format!(
                                        "Adding apartFrom constraint: {} must be ≥{}h{}m apart from {}",
                                        entity_name,
                                        time_in_minutes / 60,
                                        time_in_minutes % 60,
                                        ref_name
                                    ),
                                );
                            }

                            // This is a bit tricky to express directly in a DBM
                            // We need to express: either (entity - ref >= time) or (ref - entity >= time)
                            // For now, we'll note these constraints but not directly add them
                        }
                    }
                }
            }
        }
    }

    // Apply the "apart" constraints we collected
    for (from_var, to_var, time_minutes, description) in constraint_operations {
        compiler.add_constraint_safely(
            || Constraint::new_diff_ge(to_var, from_var, time_minutes),
            &description,
        );
    }

    // TODO: Handle the disjunctive constraints (Before/After/ApartFrom)
    // This requires a more sophisticated mechanism than what DBM directly supports

    Ok(())
}

pub fn apply_test_constraint(
    compiler: &TimeConstraintCompiler,
    test_zone: &mut clock_zones::Dbm<i64>,
    entity_name: &str,
    constraint: &ConstraintExpression,
) -> Result<(), String> {
    // Convert time value to minutes
    let time_in_minutes = constraint.time_unit.to_minutes(constraint.time_value) as i64;

    // Get all clocks for this entity
    let entity_clocks: Vec<Variable> = compiler
        .clocks
        .values()
        .filter(|c| c.entity_name == entity_name)
        .map(|c| c.variable)
        .collect();

    match &constraint.constraint_type {
        ConstraintType::Apart => {
            // Apply spacing constraint between instances of the same entity
            if entity_clocks.len() <= 1 {
                // No constraints needed for single instance
                return Ok(());
            }

            for i in 0..entity_clocks.len() {
                for j in i + 1..entity_clocks.len() {
                    // Ensure minimum spacing in either direction
                    test_zone.add_constraint(Constraint::new_diff_ge(
                        entity_clocks[i],
                        entity_clocks[j],
                        time_in_minutes,
                    ));
                    test_zone.add_constraint(Constraint::new_diff_ge(
                        entity_clocks[j],
                        entity_clocks[i],
                        time_in_minutes,
                    ));
                }
            }
        }

        ConstraintType::Before | ConstraintType::After | ConstraintType::ApartFrom => {
            // Get reference clocks based on the constraint reference
            let reference_clocks = match &constraint.reference {
                ConstraintReference::Unresolved(reference_str) => {
                    resolve_reference(compiler, reference_str)?
                }
                ConstraintReference::WithinGroup => {
                    return Err("WithinGroup reference should not be used here".to_string())
                }
            };

            for &entity_clock in &entity_clocks {
                for &reference_clock in &reference_clocks {
                    match constraint.constraint_type {
                        ConstraintType::Before => {
                            // Entity must be scheduled at least X minutes before reference
                            test_zone.add_constraint(Constraint::new_diff_ge(
                                reference_clock,
                                entity_clock,
                                time_in_minutes,
                            ));
                        }
                        ConstraintType::After => {
                            // Entity must be scheduled at least X minutes after reference
                            test_zone.add_constraint(Constraint::new_diff_ge(
                                entity_clock,
                                reference_clock,
                                time_in_minutes,
                            ));
                        }
                        ConstraintType::ApartFrom => {
                            // TODO: express constraints like 'X minutes apart from food'
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    Ok(())
}
