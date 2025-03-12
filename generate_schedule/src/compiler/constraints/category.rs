use crate::compiler::debugging::{debug_error, debug_print};
use crate::compiler::time_constraint_compiler::TimeConstraintCompiler;
use crate::types::constraints::ConstraintType;
use clock_zones::{Constraint, Variable};
use std::collections::HashMap;

// Modify apply_category_constraints in src/compiler/constraints/category.rs

pub fn apply_category_constraints(compiler: &mut TimeConstraintCompiler) -> Result<(), String> {
    // Skip if there are no category constraints
    if compiler.category_constraints.is_none()
        || compiler.category_constraints.as_ref().unwrap().is_empty()
    {
        if compiler.debug {
            debug_print(compiler, "ℹ️", "No category constraints to apply");
        }
        return Ok(());
    }

    // Create a mapping of categories to entity clocks for efficient lookup
    let mut category_entity_clocks: HashMap<String, Vec<Variable>> = HashMap::new();

    // First, group all entity clocks by their category
    for (entity_name, entity) in &compiler.entities {
        let category = entity.category.clone();

        // Get all clocks for this entity
        let entity_clocks: Vec<Variable> = compiler
            .clocks
            .values()
            .filter(|c| c.entity_name == *entity_name)
            .map(|c| c.variable)
            .collect();

        // Add entity clocks to the category map
        category_entity_clocks
            .entry(category)
            .or_default()
            .extend(entity_clocks);
    }

    // Collect disjunctive category constraints (Before OR After)
    let mut disjunctive_constraints: HashMap<
        (String, String),
        Vec<(&CategoryConstraint, ConstraintType)>,
    > = HashMap::new();

    if let Some(category_constraints) = &compiler.category_constraints {
        // First pass: identify potential disjunctive constraints
        for constraint in category_constraints {
            if constraint.constraint_type == ConstraintType::Before
                || constraint.constraint_type == ConstraintType::After
            {
                let key = (
                    constraint.from_category.clone(),
                    constraint.to_category.clone(),
                );
                disjunctive_constraints
                    .entry(key)
                    .or_default()
                    .push((constraint, constraint.constraint_type.clone()));
            }
        }
    }

    // Collect all regular constraint operations we need to perform
    let mut constraint_operations = Vec::new();

    // Process each category constraint that's not part of a disjunction
    if let Some(category_constraints) = &compiler.category_constraints {
        for constraint in category_constraints {
            let from_category = &constraint.from_category;
            let to_category = &constraint.to_category;

            // Check if this is part of a disjunctive constraint
            let key = (from_category.clone(), to_category.clone());
            let is_disjunctive = disjunctive_constraints
                .get(&key)
                .map_or(false, |constraints| {
                    let has_before = constraints
                        .iter()
                        .any(|(_, ct)| *ct == ConstraintType::Before);
                    let has_after = constraints
                        .iter()
                        .any(|(_, ct)| *ct == ConstraintType::After);
                    has_before && has_after
                });

            // Skip if part of a disjunctive constraint - we'll handle those separately
            if is_disjunctive {
                continue;
            }

            // Get clocks for both categories
            let from_clocks = category_entity_clocks.get(from_category);
            let to_clocks = category_entity_clocks.get(to_category);

            match (from_clocks, to_clocks) {
                (Some(from_vars), Some(to_vars)) => {
                    // Calculate time in minutes
                    let time_in_minutes =
                        constraint.time_unit.to_minutes(constraint.time_value) as i64;

                    match &constraint.constraint_type {
                        ConstraintType::Before => {
                            // Apply before constraints: from_category entities must be before to_category entities
                            for &from_var in from_vars {
                                for &to_var in to_vars {
                                    // Skip if same variable
                                    if from_var == to_var {
                                        continue;
                                    }

                                    let from_name =
                                        compiler.find_clock_name(from_var).unwrap_or_default();
                                    let to_name =
                                        compiler.find_clock_name(to_var).unwrap_or_default();

                                    constraint_operations.push((
                                        from_var,
                                        to_var,
                                        time_in_minutes,
                                        format!(
                                            "{} (category {}) must be ≥{}h{}m before {} (category {})",
                                            from_name,
                                            from_category,
                                            time_in_minutes / 60,
                                            time_in_minutes % 60,
                                            to_name,
                                            to_category
                                        ),
                                    ));
                                }
                            }
                        }
                        ConstraintType::After => {
                            // Apply after constraints: from_category entities must be after to_category entities
                            for &from_var in from_vars {
                                for &to_var in to_vars {
                                    // Skip if same variable
                                    if from_var == to_var {
                                        continue;
                                    }

                                    let from_name =
                                        compiler.find_clock_name(from_var).unwrap_or_default();
                                    let to_name =
                                        compiler.find_clock_name(to_var).unwrap_or_default();

                                    constraint_operations.push((
                                        to_var,
                                        from_var,
                                        time_in_minutes,
                                        format!(
                                            "{} (category {}) must be ≥{}h{}m after {} (category {})",
                                            from_name,
                                            from_category,
                                            time_in_minutes / 60,
                                            time_in_minutes % 60,
                                            to_name,
                                            to_category
                                        ),
                                    ));
                                }
                            }
                        }
                        ConstraintType::ApartFrom => {
                            // Apply apart from constraints: minimum separation between entities
                            // Note: This is more complex as we need to ensure separation in either direction
                            if compiler.debug {
                                debug_print(
                                    compiler,
                                    "ℹ️",
                                    &format!(
                                        "Category constraint: {} must be ≥{}h{}m apart from {}",
                                        from_category,
                                        time_in_minutes / 60,
                                        time_in_minutes % 60,
                                        to_category
                                    ),
                                );
                            }

                            // We don't directly add ApartFrom as a constraint here
                            // because it's a disjunctive constraint that needs special handling
                            // This would need additional logic in the DBM system
                        }
                        ConstraintType::Apart => {
                            // This type doesn't make sense for category constraints
                            debug_error(
                                compiler,
                                "⚠️",
                                &format!(
                                    "Apart constraint type not applicable for category constraints: {} and {}",
                                    from_category, to_category
                                ),
                            );
                        }
                    }
                }
                _ => {
                    debug_error(
                        compiler,
                        "⚠️",
                        &format!(
                            "Could not find clocks for categories: {} and/or {}",
                            from_category, to_category
                        ),
                    );
                }
            }
        }
    }

    // Handle disjunctive category constraints (Before OR After)
    for ((from_category, to_category), constraints) in disjunctive_constraints {
        // Only process if we have both Before and After constraints
        let before_constraints: Vec<_> = constraints
            .iter()
            .filter(|(_, ct)| *ct == ConstraintType::Before)
            .collect();

        let after_constraints: Vec<_> = constraints
            .iter()
            .filter(|(_, ct)| *ct == ConstraintType::After)
            .collect();

        if !before_constraints.is_empty() && !after_constraints.is_empty() {
            // We have a disjunctive constraint (Before OR After)
            if compiler.debug {
                debug_print(
                    compiler,
                    "ℹ️",
                    &format!(
                        "Detected disjunctive category constraint between {} and {}",
                        from_category, to_category
                    ),
                );
            }

            // For simplicity, take the first of each constraint type
            let (before_constraint, _) = before_constraints[0];
            let (after_constraint, _) = after_constraints[0];

            let before_minutes = before_constraint
                .time_unit
                .to_minutes(before_constraint.time_value) as i64;
            let after_minutes = after_constraint
                .time_unit
                .to_minutes(after_constraint.time_value) as i64;

            // Get clocks for both categories
            if let (Some(from_vars), Some(to_vars)) = (
                category_entity_clocks.get(&from_category),
                category_entity_clocks.get(&to_category),
            ) {
                // Try disjunctive constraints for each from-to clock pair
                for &from_var in from_vars {
                    for &to_var in to_vars {
                        if from_var == to_var {
                            continue;
                        }

                        let from_name = compiler.find_clock_name(from_var).unwrap_or_default();
                        let to_name = compiler.find_clock_name(to_var).unwrap_or_default();

                        // Define both constraints for the disjunction
                        let before_constraint_func =
                            || Constraint::new_diff_ge(to_var, from_var, before_minutes);
                        let before_desc = format!(
                            "{} (category {}) must be ≥{}h{}m before {} (category {})",
                            from_name,
                            from_category,
                            before_minutes / 60,
                            before_minutes % 60,
                            to_name,
                            to_category
                        );

                        let after_constraint_func =
                            || Constraint::new_diff_ge(from_var, to_var, after_minutes);
                        let after_desc = format!(
                            "{} (category {}) must be ≥{}h{}m after {} (category {})",
                            from_name,
                            from_category,
                            after_minutes / 60,
                            after_minutes % 60,
                            to_name,
                            to_category
                        );

                        // Try the disjunctive constraint
                        compiler.try_disjunction(
                            before_constraint_func,
                            &before_desc,
                            after_constraint_func,
                            &after_desc,
                        );
                    }
                }
            }
        }
    }

    // Apply the regular constraints we collected
    for (from_var, to_var, time_minutes, description) in constraint_operations {
        compiler.add_constraint_safely(
            || Constraint::new_diff_ge(to_var, from_var, time_minutes),
            &description,
        );
    }

    // Handle ApartFrom category constraints
    handle_category_apart_from(compiler)?;

    Ok(())
}

pub fn handle_category_apart_from(compiler: &mut TimeConstraintCompiler) -> Result<(), String> {
    // Skip if there are no category constraints
    if compiler.category_constraints.is_none() {
        return Ok(());
    }

    // Create a mapping of categories to entity clocks for efficient lookup
    let mut category_entity_clocks: HashMap<String, Vec<Variable>> = HashMap::new();

    // Group all entity clocks by their category
    for (entity_name, entity) in &compiler.entities {
        let category = entity.category.clone();

        // Get all clocks for this entity
        let entity_clocks: Vec<Variable> = compiler
            .clocks
            .values()
            .filter(|c| c.entity_name == *entity_name)
            .map(|c| c.variable)
            .collect();

        // Add entity clocks to the category map
        category_entity_clocks
            .entry(category)
            .or_default()
            .extend(entity_clocks);
    }

    // Process ApartFrom constraints
    if let Some(category_constraints) = &compiler.category_constraints {
        for constraint in category_constraints {
            if constraint.constraint_type != ConstraintType::ApartFrom {
                continue;
            }

            let from_category = &constraint.from_category;
            let to_category = &constraint.to_category;
            let time_in_minutes = constraint.time_unit.to_minutes(constraint.time_value) as i64;

            // Get clocks for both categories
            if let (Some(from_vars), Some(to_vars)) = (
                category_entity_clocks.get(from_category),
                category_entity_clocks.get(to_category),
            ) {
                // For each pair of clocks between the categories, apply disjunctive constraint
                for &from_var in from_vars {
                    for &to_var in to_vars {
                        if from_var == to_var {
                            continue;
                        }

                        let from_name = compiler.find_clock_name(from_var).unwrap_or_default();
                        let to_name = compiler.find_clock_name(to_var).unwrap_or_default();

                        // Define two disjunctive constraints:
                        // 1. From category is before To category
                        let from_before_to =
                            || Constraint::new_diff_ge(to_var, from_var, time_in_minutes);
                        let from_before_to_desc = format!(
                            "{} (category {}) must be ≥{}h{}m before {} (category {})",
                            from_name,
                            from_category,
                            time_in_minutes / 60,
                            time_in_minutes % 60,
                            to_name,
                            to_category
                        );

                        // 2. From category is after To category
                        let to_before_from =
                            || Constraint::new_diff_ge(from_var, to_var, time_in_minutes);
                        let to_before_from_desc = format!(
                            "{} (category {}) must be ≥{}h{}m after {} (category {})",
                            from_name,
                            from_category,
                            time_in_minutes / 60,
                            time_in_minutes % 60,
                            to_name,
                            to_category
                        );

                        // Try the disjunctive constraint
                        compiler.try_disjunction(
                            from_before_to,
                            &from_before_to_desc,
                            to_before_from,
                            &to_before_from_desc,
                        );
                    }
                }
            }
        }
    }

    Ok(())
}
// Add this at the end of src/compiler/constraints/category.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::constraints::{CategoryConstraint, ConstraintType};
    use crate::types::entity::Entity;
    use crate::types::frequency::FrequencyType;
    use crate::types::time_unit::TimeUnit;

    #[test]
    fn test_category_disjunctive_constraints() {
        // Create test entities
        let entity1 = Entity::new(
            "medication".to_string(),
            "medicine".to_string(),
            FrequencyType::TwiceDaily,
        );
        let entity2 = Entity::new(
            "meal".to_string(),
            "food".to_string(),
            FrequencyType::TwiceDaily,
        );

        // Create compiler
        let mut compiler = TimeConstraintCompiler::new(vec![entity1, entity2]);

        // Add category-level disjunctive constraints
        // Medicine category must be either ≥2h before food category OR ≥1h after food category
        let mut category_constraints = Vec::new();

        category_constraints.push(CategoryConstraint::new(
            "medicine".to_string(),
            "food".to_string(),
            ConstraintType::Before,
            2,
            TimeUnit::Hour,
        ));

        category_constraints.push(CategoryConstraint::new(
            "medicine".to_string(),
            "food".to_string(),
            ConstraintType::After,
            1,
            TimeUnit::Hour,
        ));

        compiler.set_category_constraints(category_constraints);

        // Compile and check if it's feasible
        let result = compiler.compile();
        assert!(
            result.is_ok(),
            "Schedule should be feasible with category disjunctive constraints"
        );

        // Extract schedule
        let schedule = compiler.extract_schedule().unwrap();

        // Check that all medications and meals are scheduled
        assert_eq!(schedule.len(), 4); // 2 medications + 2 meals

        // Similar checks as above for each medication-meal pair
        let med1_time = schedule.get("medication_1").unwrap();
        let med2_time = schedule.get("medication_2").unwrap();
        let meal1_time = schedule.get("meal_1").unwrap();
        let meal2_time = schedule.get("meal_2").unwrap();

        // Helper function to check if constraints are satisfied
        let check_constraints = |med_time: &i32, meal_time: &i32| -> bool {
            let before_satisfied = meal_time - med_time >= 120; // 2h = 120 minutes
            let after_satisfied = med_time - meal_time >= 60; // 1h = 60 minutes
            before_satisfied || after_satisfied
        };

        // Verify constraints for all medication-meal pairs
        assert!(
            check_constraints(med1_time, meal1_time),
            "Medication 1 and Meal 1 should satisfy category disjunctive constraints"
        );

        assert!(
            check_constraints(med1_time, meal2_time),
            "Medication 1 and Meal 2 should satisfy category disjunctive constraints"
        );

        assert!(
            check_constraints(med2_time, meal1_time),
            "Medication 2 and Meal 1 should satisfy category disjunctive constraints"
        );

        assert!(
            check_constraints(med2_time, meal2_time),
            "Medication 2 and Meal 2 should satisfy category disjunctive constraints"
        );
    }

    #[test]
    fn test_category_apart_from() {
        // Create test entities
        let entity1 = Entity::new(
            "medication".to_string(),
            "medicine".to_string(),
            FrequencyType::TwiceDaily,
        );
        let entity2 = Entity::new(
            "meal".to_string(),
            "food".to_string(),
            FrequencyType::TwiceDaily,
        );

        // Create compiler
        let mut compiler = TimeConstraintCompiler::new(vec![entity1, entity2]);

        // Add category ApartFrom constraint
        let mut category_constraints = Vec::new();
        category_constraints.push(CategoryConstraint::new(
            "medicine".to_string(),
            "food".to_string(),
            ConstraintType::ApartFrom,
            2,
            TimeUnit::Hour,
        ));

        compiler.set_category_constraints(category_constraints);

        // Compile and check if it's feasible
        let result = compiler.compile();
        assert!(
            result.is_ok(),
            "Schedule should be feasible with category ApartFrom constraints"
        );

        // Extract schedule
        let schedule = compiler.extract_schedule().unwrap();

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
