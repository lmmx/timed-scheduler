use crate::compiler::clock_info::ClockInfo;
use crate::compiler::debugging::{debug_error, debug_print};
use crate::compiler::reference_resolution::resolve_reference;
use crate::compiler::time_constraint_compiler::{DisjunctiveOp, TimeConstraintCompiler};
use crate::types::constraints::{ConstraintExpression, ConstraintReference, ConstraintType};
use crate::types::time_unit::TimeUnit;
use crate::types::time_unit::TimeUnit::Hour;
use clock_zones::{Constraint, Variable, Zone};
use std::collections::HashMap;

pub fn apply_entity_constraints(compiler: &mut TimeConstraintCompiler) -> Result<(), String> {
    // First, collect all constraint operations we need to perform
    let mut constraint_operations = Vec::new();

    // Track disjunctive constraints (Before OR After) for the same entity-reference pair
    let mut disjunctive_constraints: HashMap<
        (String, String),
        Vec<(ConstraintType, u32, TimeUnit)>,
    > = HashMap::new();

    // Build our entity clock map up front
    let mut entity_clocks_map = std::collections::HashMap::new();
    for (entity_name, _) in &compiler.entities {
        let entity_clocks: Vec<&ClockInfo> = compiler
            .clocks
            .values()
            .filter(|c| c.entity_name == *entity_name)
            .collect();
        entity_clocks_map.insert(entity_name.clone(), entity_clocks);
    }

    // First pass: identify potential disjunctive constraints (before OR after)
    for (entity_name, entity) in &compiler.entities {
        for constraint in &entity.constraints {
            if let ConstraintType::Before | ConstraintType::After = constraint.constraint_type {
                if let ConstraintReference::Unresolved(ref_str) = &constraint.reference {
                    let key = (entity_name.clone(), ref_str.clone());
                    disjunctive_constraints.entry(key).or_default().push((
                        constraint.constraint_type.clone(),
                        constraint.time_value,
                        constraint.time_unit.clone(),
                    ));
                }
            }
        }
    }

    // Second pass: process all constraints
    for (entity_name, entity) in &compiler.entities {
        let entity_clocks = entity_clocks_map.get(entity_name).unwrap();

        // Process all constraint types for this entity
        for constraint in &entity.constraints {
            match &constraint.constraint_type {
                ConstraintType::Apart => {
                    // No change to Apart handling
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

                    // Check if this is part of a disjunctive constraint
                    let key = (entity_name.clone(), reference_str.clone());
                    let is_disjunctive =
                        disjunctive_constraints
                            .get(&key)
                            .map_or(false, |constraints| {
                                let has_before = constraints
                                    .iter()
                                    .any(|(ct, _, _)| *ct == ConstraintType::Before);
                                let has_after = constraints
                                    .iter()
                                    .any(|(ct, _, _)| *ct == ConstraintType::After);
                                has_before && has_after
                            });

                    // If it's part of a disjunctive constraint (Before OR After),
                    // we'll handle it separately and skip adding it to constraint_operations here
                    if is_disjunctive {
                        continue;
                    }

                    // Get reference clocks based on the constraint reference
                    let reference_clocks = match &constraint.reference {
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

                    let time_in_minutes =
                        constraint.time_unit.to_minutes(constraint.time_value) as i64;

                    // Apply Before/After constraints by iterating through all entity clocks and reference clocks
                    let entity_vars: Vec<Variable> =
                        entity_clocks.iter().map(|c| c.variable).collect();
                    for entity_var in entity_vars {
                        for &reference_var in &reference_clocks {
                            // Skip if same variable
                            if entity_var == reference_var {
                                continue;
                            }

                            let entity_clock_name =
                                compiler.find_clock_name(entity_var).unwrap_or_default();
                            let reference_clock_name =
                                compiler.find_clock_name(reference_var).unwrap_or_default();

                            match constraint.constraint_type {
                                ConstraintType::Before => {
                                    // Entity must be before reference
                                    constraint_operations.push((
                                        entity_var,
                                        reference_var,
                                        time_in_minutes,
                                        format!(
                                            "{} must be ≥{}h{}m before {}",
                                            entity_clock_name,
                                            time_in_minutes / 60,
                                            time_in_minutes % 60,
                                            reference_clock_name
                                        ),
                                    ));
                                }
                                ConstraintType::After => {
                                    // Entity must be after reference
                                    constraint_operations.push((
                                        reference_var,
                                        entity_var,
                                        time_in_minutes,
                                        format!(
                                            "{} must be ≥{}h{}m after {}",
                                            entity_clock_name,
                                            time_in_minutes / 60,
                                            time_in_minutes % 60,
                                            reference_clock_name
                                        ),
                                    ));
                                }
                                _ => unreachable!(),
                            }
                        }
                    }

                    // For debugging
                    if compiler.debug {
                        debug_print(
                            compiler,
                            "ℹ️",
                            &format!(
                                "Applied {} constraint: {} must be ≥{}{}m {} {}",
                                match constraint.constraint_type {
                                    ConstraintType::Before => "before",
                                    ConstraintType::After => "after",
                                    _ => "related to",
                                },
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
                    // Keep existing ApartFrom handling
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

                    // For "apart from", we note these constraints but don't directly add them
                    // as they require disjunctive constraints (either A-B>=time OR B-A>=time)
                    // This is difficult to express directly in a DBM

                    for entity_var in entity_clocks.iter().map(|c| c.variable) {
                        for &reference_var in &reference_clocks {
                            // Skip if same variable
                            if entity_var == reference_var {
                                continue;
                            }

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
                        }
                    }
                }
            }
        }
    }

    // Handle disjunctive constraints (Before OR After for the same entity-reference pair)
    for ((entity_name, reference_str), constraints) in disjunctive_constraints {
        // Only process if we have both Before and After constraints
        let before_constraints: Vec<_> = constraints
            .iter()
            .filter(|(ct, _, _)| *ct == ConstraintType::Before)
            .collect();

        let after_constraints: Vec<_> = constraints
            .iter()
            .filter(|(ct, _, _)| *ct == ConstraintType::After)
            .collect();

        if !before_constraints.is_empty() && !after_constraints.is_empty() {
            // We have a disjunctive constraint
            if compiler.debug {
                debug_print(
                    compiler,
                    "ℹ️",
                    &format!(
                        "Detected disjunctive constraint for {} and {}",
                        entity_name, reference_str
                    ),
                );
            }

            // For simplicity, just take the first of each constraint type
            let (_, before_time, before_unit) = before_constraints[0];
            let (_, after_time, after_unit) = after_constraints[0];

            let before_minutes = before_unit.to_minutes(*before_time) as i64;
            let after_minutes = after_unit.to_minutes(*after_time) as i64;

            // Get entity and reference clocks
            let entity_clocks = match entity_clocks_map.get(&entity_name) {
                Some(clocks) => clocks.iter().map(|c| c.variable).collect::<Vec<_>>(),
                None => continue,
            };

            let reference_clocks = match resolve_reference(compiler, &reference_str) {
                Ok(clocks) => clocks,
                Err(_) => continue,
            };

            // Try disjunctive constraints for each entity-reference clock pair
            for &entity_var in &entity_clocks {
                for &reference_var in &reference_clocks {
                    if entity_var == reference_var {
                        continue;
                    }

                    let entity_clock_name =
                        compiler.find_clock_name(entity_var).unwrap_or_default();
                    let reference_clock_name =
                        compiler.find_clock_name(reference_var).unwrap_or_default();

                    // Define both constraints for the disjunction
                    let before_desc = format!(
                        "{} must be ≥{}h{}m before {}",
                        entity_clock_name,
                        before_minutes / 60,
                        before_minutes % 60,
                        reference_clock_name
                    );

                    let after_desc = format!(
                        "{} must be ≥{}h{}m after {}",
                        entity_clock_name,
                        after_minutes / 60,
                        after_minutes % 60,
                        reference_clock_name
                    );

                    compiler.disjunctive_ops.push(DisjunctiveOp {
                        var1: reference_var,
                        var2: entity_var,
                        time1: before_minutes,
                        desc1: before_desc.clone(),
                        var3: entity_var,
                        var4: reference_var,
                        time2: after_minutes,
                        desc2: after_desc.clone(),
                    });
                }
            }
        }
    }

    // Apply the regular constraints we collected
    for (from_var, to_var, time_minutes, description) in constraint_operations {
        if description.starts_with("Special constraint:") {
            // Remove any special case handling tied to specific entities
            continue;
        }

        compiler.add_constraint_safely(
            || Constraint::new_diff_ge(to_var, from_var, time_minutes),
            &description,
        );
    }

    // Handle ApartFrom constraints with our disjunctive approach
    handle_apart_from_constraints(compiler)?;

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
                    test_zone.add_constraints([Constraint::new_diff_ge(
                        entity_clocks[i],
                        entity_clocks[j],
                        time_in_minutes,
                    )]);
                    test_zone.add_constraints([Constraint::new_diff_ge(
                        entity_clocks[j],
                        entity_clocks[i],
                        time_in_minutes,
                    )]);
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
                            test_zone.add_constraints([Constraint::new_diff_ge(
                                reference_clock,
                                entity_clock,
                                time_in_minutes,
                            )]);
                        }
                        ConstraintType::After => {
                            // Entity must be scheduled at least X minutes after reference
                            test_zone.add_constraints([Constraint::new_diff_ge(
                                entity_clock,
                                reference_clock,
                                time_in_minutes,
                            )]);
                        }
                        ConstraintType::ApartFrom => {
                            // For testing, we can at least check one direction
                            // In a real solution, we would need to handle disjunctive constraints
                            test_zone.add_constraints([Constraint::new_diff_ge(
                                entity_clock,
                                reference_clock,
                                time_in_minutes,
                            )]);
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn handle_apart_from_constraints(compiler: &mut TimeConstraintCompiler) -> Result<(), String> {
    // Track all ApartFrom constraints
    let mut apart_from_constraints = Vec::new();

    // First, identify all ApartFrom constraints
    for (entity_name, entity) in &compiler.entities {
        for constraint in &entity.constraints {
            if constraint.constraint_type == ConstraintType::ApartFrom {
                if let ConstraintReference::Unresolved(reference_str) = &constraint.reference {
                    apart_from_constraints.push((
                        entity_name.clone(),
                        reference_str.clone(),
                        constraint.time_value,
                        constraint.time_unit.clone(),
                    ));
                }
            }
        }
    }

    // Get all clocks for each entity
    let mut entity_clocks_map = HashMap::new();
    for (entity_name, _) in &compiler.entities {
        let entity_clocks: Vec<Variable> = compiler
            .clocks
            .values()
            .filter(|c| c.entity_name == *entity_name)
            .map(|c| c.variable)
            .collect();
        entity_clocks_map.insert(entity_name.clone(), entity_clocks);
    }

    // Process each ApartFrom constraint
    for (entity_name, reference_str, time_value, time_unit) in apart_from_constraints {
        let time_in_minutes = time_unit.to_minutes(time_value) as i64;

        // Get entity clocks
        let entity_clocks = match entity_clocks_map.get(&entity_name) {
            Some(clocks) => clocks,
            None => continue,
        };

        // Get reference clocks
        let reference_clocks = match resolve_reference(compiler, &reference_str) {
            Ok(clocks) => clocks,
            Err(e) => {
                debug_error(
                    compiler,
                    "⚠️",
                    &format!("Could not resolve reference '{}': {}", reference_str, e),
                );
                continue;
            }
        };

        // For each entity-reference clock pair, create a disjunctive constraint
        for &entity_var in entity_clocks {
            for &reference_var in &reference_clocks {
                // Skip if same variable
                if entity_var == reference_var {
                    continue;
                }

                let entity_name = compiler.find_clock_name(entity_var).unwrap_or_default();
                let ref_name = compiler.find_clock_name(reference_var).unwrap_or_default();

                // Define the two disjunctive constraints:
                // 1. Entity at least time_in_minutes before reference
                let entity_before =
                    || Constraint::new_diff_ge(reference_var, entity_var, time_in_minutes);
                let entity_before_desc = format!(
                    "{} must be ≥{}h{}m before {}",
                    entity_name,
                    time_in_minutes / 60,
                    time_in_minutes % 60,
                    ref_name
                );

                // 2. Entity at least time_in_minutes after reference
                let entity_after =
                    || Constraint::new_diff_ge(entity_var, reference_var, time_in_minutes);
                let entity_after_desc = format!(
                    "{} must be ≥{}h{}m after {}",
                    entity_name,
                    time_in_minutes / 60,
                    time_in_minutes % 60,
                    ref_name
                );

                // Try the disjunctive constraint
                compiler.try_disjunction(
                    entity_before,
                    &entity_before_desc,
                    entity_after,
                    &entity_after_desc,
                );
            }
        }
    }

    Ok(())
}
// Add this at the end of src/compiler/constraints/entity.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::entity::Entity;
    use crate::types::frequency::FrequencyType;
    use crate::types::time_unit::TimeUnit;

    #[test]
    fn test_entity_disjunctive_constraints() {
        // Create test entities
        let mut entity1 = Entity::new(
            "medication".to_string(),
            "medicine".to_string(),
            FrequencyType::TwiceDaily,
        );
        let mut entity2 = Entity::new(
            "meal".to_string(),
            "food".to_string(),
            FrequencyType::TwiceDaily,
        );

        // Add disjunctive constraints
        // Medication must be either ≥2h before food OR ≥1h after food
        entity1.add_constraint("≥2h before food").unwrap();
        entity1.add_constraint("≥1h after food").unwrap();

        // Create compiler
        let mut compiler = TimeConstraintCompiler::new(vec![entity1, entity2]);

        // Compile and check if it's feasible
        let result = compiler.compile();
        assert!(
            result.is_ok(),
            "Schedule should be feasible with disjunctive constraints"
        );

        // Extract schedule
        let schedule = compiler.extract_schedule().unwrap();

        // Check that all medications and meals are scheduled
        assert_eq!(schedule.len(), 4); // 2 medications + 2 meals

        // Verify that the disjunctive constraints are satisfied
        let med1_time = schedule.get("medication_1").unwrap();
        let med2_time = schedule.get("medication_2").unwrap();
        let meal1_time = schedule.get("meal_1").unwrap();
        let meal2_time = schedule.get("meal_2").unwrap();

        // For each medication-meal pair, check that EITHER:
        // 1. Medication is ≥2h before meal, OR
        // 2. Medication is ≥1h after meal

        // Helper function to check if constraints are satisfied
        let check_constraints = |med_time: &i32, meal_time: &i32| -> bool {
            let before_satisfied = meal_time - med_time >= 120; // 2h = 120 minutes
            let after_satisfied = med_time - meal_time >= 60; // 1h = 60 minutes
            before_satisfied || after_satisfied
        };

        assert!(
            check_constraints(med1_time, meal1_time),
            "Medication 1 and Meal 1 should satisfy disjunctive constraints"
        );

        assert!(
            check_constraints(med1_time, meal2_time),
            "Medication 1 and Meal 2 should satisfy disjunctive constraints"
        );

        assert!(
            check_constraints(med2_time, meal1_time),
            "Medication 2 and Meal 1 should satisfy disjunctive constraints"
        );

        assert!(
            check_constraints(med2_time, meal2_time),
            "Medication 2 and Meal 2 should satisfy disjunctive constraints"
        );
    }

    #[test]
    fn test_apart_from_constraints() {
        // Create test entities
        let mut entity1 = Entity::new(
            "medication".to_string(),
            "medicine".to_string(),
            FrequencyType::TwiceDaily,
        );
        let mut entity2 = Entity::new(
            "meal".to_string(),
            "food".to_string(),
            FrequencyType::TwiceDaily,
        );

        // Add ApartFrom constraint
        // Medication must be ≥2h apart from food (either before OR after)
        entity1.add_constraint("≥2h apart from food").unwrap();

        // Create compiler
        let mut compiler = TimeConstraintCompiler::new(vec![entity1, entity2]);

        // Compile and check if it's feasible
        let result = compiler.compile();
        assert!(
            result.is_ok(),
            "Schedule should be feasible with ApartFrom constraints"
        );

        // Extract schedule
        let schedule = compiler.extract_schedule().unwrap();

        // Check that all medications and meals are scheduled
        assert_eq!(schedule.len(), 4); // 2 medications + 2 meals

        // Verify that the ApartFrom constraints are satisfied
        let med1_time = schedule.get("medication_1").unwrap();
        let med2_time = schedule.get("medication_2").unwrap();
        let meal1_time = schedule.get("meal_1").unwrap();
        let meal2_time = schedule.get("meal_2").unwrap();

        // Helper function to check if ApartFrom constraints are satisfied
        let check_apart_from = |time1: &i32, time2: &i32| -> bool {
            (time2 - time1).abs() >= 120 // 2h = 120 minutes
        };

        // Verify constraints for all medication-meal pairs
        assert!(
            check_apart_from(med1_time, meal1_time),
            "Medication 1 and Meal 1 should be at least 2h apart"
        );

        assert!(
            check_apart_from(med1_time, meal2_time),
            "Medication 1 and Meal 2 should be at least 2h apart"
        );

        assert!(
            check_apart_from(med2_time, meal1_time),
            "Medication 2 and Meal 1 should be at least 2h apart"
        );

        assert!(
            check_apart_from(med2_time, meal2_time),
            "Medication 2 and Meal 2 should be at least 2h apart"
        );
    }
}
