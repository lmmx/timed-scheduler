use crate::compiler::debugging::{debug_error, debug_print};
use crate::compiler::time_constraint_compiler::TimeConstraintCompiler;
use crate::types::constraints::{CategoryConstraint, ConstraintType};
use std::collections::HashMap;
use clock_zones::{Constraint, Variable, Zone};

pub fn apply_category_constraints(compiler: &mut TimeConstraintCompiler) -> Result<(), String> {
    // Skip if there are no category constraints
    if compiler.category_constraints.is_none() || compiler.category_constraints.as_ref().unwrap().is_empty() {
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

    // Collect all constraint operations we need to perform
    let mut constraint_operations = Vec::new();

    // Process each category constraint
    if let Some(category_constraints) = &compiler.category_constraints {
        for constraint in category_constraints {
            let from_category = &constraint.from_category;
            let to_category = &constraint.to_category;
            
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
                                    
                                    let from_name = compiler.find_clock_name(from_var).unwrap_or_default();
                                    let to_name = compiler.find_clock_name(to_var).unwrap_or_default();
                                    
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
                        },
                        ConstraintType::After => {
                            // Apply after constraints: from_category entities must be after to_category entities
                            for &from_var in from_vars {
                                for &to_var in to_vars {
                                    // Skip if same variable
                                    if from_var == to_var {
                                        continue;
                                    }
                                    
                                    let from_name = compiler.find_clock_name(from_var).unwrap_or_default();
                                    let to_name = compiler.find_clock_name(to_var).unwrap_or_default();
                                    
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
                        },
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
                        },
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
                },
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

    // Apply all the constraints we've collected
    for (from_var, to_var, time_minutes, description) in constraint_operations {
        compiler.add_constraint_safely(
            || Constraint::new_diff_ge(to_var, from_var, time_minutes),
            &description,
        );
    }

    Ok(())
}

pub fn apply_test_category_constraint(
    compiler: &TimeConstraintCompiler,
    test_zone: &mut clock_zones::Dbm<i64>,
    constraint: &CategoryConstraint,
) -> Result<(), String> {
    // Create a mapping of categories to entity clocks for lookup
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

    // Get clocks for both categories
    let from_clocks = match category_entity_clocks.get(&constraint.from_category) {
        Some(clocks) => clocks,
        None => return Err(format!("Category not found: {}", constraint.from_category)),
    };
    
    let to_clocks = match category_entity_clocks.get(&constraint.to_category) {
        Some(clocks) => clocks,
        None => return Err(format!("Category not found: {}", constraint.to_category)),
    };

    // Calculate time in minutes
    let time_in_minutes = constraint.time_unit.to_minutes(constraint.time_value) as i64;
    
    match &constraint.constraint_type {
        ConstraintType::Before => {
            // Apply before constraints
            for &from_var in from_clocks {
                for &to_var in to_clocks {
                    if from_var == to_var {
                        continue;
                    }
                    test_zone.add_constraints([Constraint::new_diff_ge(
                        to_var,
                        from_var,
                        time_in_minutes,
                    )]);
                }
            }
        },
        ConstraintType::After => {
            // Apply after constraints
            for &from_var in from_clocks {
                for &to_var in to_clocks {
                    if from_var == to_var {
                        continue;
                    }
                    test_zone.add_constraints([Constraint::new_diff_ge(
                        from_var,
                        to_var,
                        time_in_minutes,
                    )]);
                }
            }
        },
        ConstraintType::ApartFrom => {
            // This is a more complex constraint that would need special handling
            // TODO: Implement proper ApartFrom logic for categories
        },
        ConstraintType::Apart => {
            // Not applicable for category constraints
        }
    }

    Ok(())
}