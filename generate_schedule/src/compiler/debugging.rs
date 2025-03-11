use crate::compiler::constraints::entity::apply_test_constraint;
use crate::compiler::time_constraint_compiler::TimeConstraintCompiler;
use clock_zones::{Bound, Clock, Constraint, Dbm, Variable};
use colored::*;
use std::collections::HashMap;

pub fn debug_print(compiler: &TimeConstraintCompiler, emoji: &str, message: &str) {
    if compiler.debug {
        println!("{} {}", emoji.green(), message.bright_blue());
    }
}

pub fn debug_error(compiler: &TimeConstraintCompiler, emoji: &str, message: &str) {
    if compiler.debug {
        println!("{} {}", emoji.red(), message.bright_red());
    }
}

pub fn debug_zone_state(compiler: &TimeConstraintCompiler) {
    if !compiler.debug {
        return;
    }

    println!("{}", "🔍 Current Zone State:".yellow().bold());

    if compiler.zone.is_empty() {
        println!("{}", "   ❌ ZONE IS EMPTY (infeasible)".red().bold());
        return;
    }

    println!("{}", "   ✅ ZONE IS FEASIBLE".green().bold());

    // Print bounds for each clock
    for (clock_id, clock_info) in &compiler.clocks {
        let lower = compiler.zone.get_lower_bound(clock_info.variable);
        let upper = compiler.zone.get_upper_bound(clock_info.variable);

        let bounds_str = match (lower, upper) {
            (Some(l), Some(u)) => {
                let l_hour = l / 60;
                let l_min = l % 60;
                let u_hour = u / 60;
                let u_min = u % 60;
                format!("[{:02}:{:02} - {:02}:{:02}]", l_hour, l_min, u_hour, u_min)
            }
            _ => "[unknown bounds]".to_string(),
        };

        println!(
            "   {} ({}): {}",
            clock_id.cyan(),
            clock_info.entity_name.blue(),
            bounds_str.yellow()
        );
    }

    // Print some difference constraints
    println!("{}", "   Difference Constraints (sample):".yellow());
    let mut constraints_shown = 0;

    for i in 0..compiler.clocks.len() {
        for j in 0..compiler.clocks.len() {
            if i == j {
                continue;
            }

            let var_i = Clock::variable(i);
            let var_j = Clock::variable(j);

            let bound = compiler.zone.get_bound(var_i, var_j);
            if let Some(diff) = bound.constant() {
                if constraints_shown < 5 {
                    // Limit to 5 constraints to avoid overwhelming output
                    let name_i = compiler
                        .find_clock_name(var_i)
                        .unwrap_or_else(|| "unknown".to_string());
                    let name_j = compiler
                        .find_clock_name(var_j)
                        .unwrap_or_else(|| "unknown".to_string());

                    println!(
                        "     {} - {} <= {} ({} minutes)",
                        name_i.green(),
                        name_j.green(),
                        diff.to_string().yellow(),
                        diff.to_string().yellow()
                    );

                    constraints_shown += 1;
                }
            }
        }
    }

    println!();
}

pub fn diagnose_infeasibility<B: clock_zones::Bound<Constant = i32>>(
    compiler: &mut TimeConstraintCompiler,
) {
    if !compiler.debug {
        return;
    }

    debug_print(
        compiler,
        "🔎",
       "Running diagnosis to find problematic constraints"),
    ;

    // Try with just daily bounds
    let mut test_zone = Dbm::<B>::new_zero(compiler.next_clock_index);

    // Apply only daily bounds
    for clock_info in compiler.clocks.values() {
        test_zone.add_constraint(Constraint::new_ge(clock_info.variable, 0));
        test_zone.add_constraint(Constraint::new_le(clock_info.variable, 1440));
    }

    if test_zone.is_empty() {
        debug_error(
            compiler,
            "⚠️",
            "Even basic daily bounds (0-1440) lead to infeasibility!",
        );
        return;
    }

    debug_print(compiler, "✓", "Basic daily bounds are feasible");

    // Try with frequency constraints
    let mut test_zone = Dbm::new_zero(compiler.next_clock_index);

    // Apply daily bounds
    for clock_info in compiler.clocks.values() {
        test_zone.add_constraint(Constraint::new_ge(clock_info.variable, 0));
        test_zone.add_constraint(Constraint::new_le(clock_info.variable, 1440));
    }

    // Group clocks by entity
    let mut entity_clocks: HashMap<String, Vec<Variable>> = HashMap::new();
    for clock_info in compiler.clocks.values() {
        entity_clocks
            .entry(clock_info.entity_name.clone())
            .or_default()
            .push(clock_info.variable);
    }

    // Apply ordering constraints only
    for (entity_name, clocks) in &entity_clocks {
        if clocks.len() <= 1 {
            continue;
        }

        let mut ordered_clocks: Vec<(usize, Variable)> = compiler
            .clocks
            .values()
            .filter(|c| c.entity_name == *entity_name)
            .map(|c| (c.instance, c.variable))
            .collect();
        ordered_clocks.sort_by_key(|&(instance, _)| instance);

        for i in 0..ordered_clocks.len() - 1 {
            let (_, current) = ordered_clocks[i];
            let (_, next) = ordered_clocks[i + 1];

            // Next instance must come after current instance
            test_zone.add_constraint(Constraint::new_diff_gt(next, current, 0));
        }
    }

    if test_zone.is_empty() {
        debug_error(
            compiler,
            "⚠️",
            "Ordering constraints lead to infeasibility!",
        );
        return;
    }

    debug_print(compiler, "✓", "Basic ordering constraints are feasible");

    // Now try applying spacing constraints
    for (entity_name, clocks) in &entity_clocks {
        if clocks.len() <= 1 {
            continue;
        }

        let entity = compiler.entities.get(entity_name).unwrap();

        let mut ordered_clocks: Vec<(usize, Variable)> = compiler
            .clocks
            .values()
            .filter(|c| c.entity_name == *entity_name)
            .map(|c| (c.instance, c.variable))
            .collect();
        ordered_clocks.sort_by_key(|&(instance, _)| instance);

        let mut test_zone_with_spacing = test_zone.clone();
        let min_spacing = match entity.frequency {
            crate::types::frequency::Frequency::TwiceDaily => 6 * 60, // 6 hours in minutes
            crate::types::frequency::Frequency::ThreeTimesDaily => 4 * 60, // 4 hours in minutes
            crate::types::frequency::Frequency::EveryXHours(hours) => (hours as i64) * 60,
            _ => 60, // Default 1 hour minimum spacing
        };

        for i in 0..ordered_clocks.len() - 1 {
            let (_, current) = ordered_clocks[i];
            let (_, next) = ordered_clocks[i + 1];

            test_zone_with_spacing.add_constraint(Constraint::new_diff_ge(
                next,
                current,
                min_spacing,
            ));
        }

        if test_zone_with_spacing.is_empty() {
            debug_error(
                compiler,
                "⚠️",
                &format!(
                    "Spacing constraints for '{}' (≥{} min) lead to infeasibility!",
                    entity_name, min_spacing
                ),
            );
        }
    }

    // Try individual entity constraints
    let mut problem_constraints = Vec::new();

    for (entity_name, entity) in &compiler.entities {
        for constraint in &entity.constraints {
            let mut test_zone_with_constraint = test_zone.clone();

            match apply_test_constraint(
                compiler,
                &mut test_zone_with_constraint,
                entity_name,
                constraint,
            ) {
                Ok(_) => {
                    if test_zone_with_constraint.is_empty() {
                        let constraint_str = match &constraint.constraint_type {
                            crate::types::constraints::ConstraintType::Before => format!(
                                "≥{}{} before {:?}",
                                constraint.time_value,
                                if constraint.time_unit == crate::types::time_unit::TimeUnit::Hour {
                                    "h"
                                } else {
                                    "m"
                                },
                                constraint.reference
                            ),
                            crate::types::constraints::ConstraintType::After => format!(
                                "≥{}{} after {:?}",
                                constraint.time_value,
                                if constraint.time_unit == crate::types::time_unit::TimeUnit::Hour {
                                    "h"
                                } else {
                                    "m"
                                },
                                constraint.reference
                            ),
                            crate::types::constraints::ConstraintType::ApartFrom => format!(
                                "≥{}{} apart from {:?}",
                                constraint.time_value,
                                if constraint.time_unit == crate::types::time_unit::TimeUnit::Hour {
                                    "h"
                                } else {
                                    "m"
                                },
                                constraint.reference
                            ),
                            crate::types::constraints::ConstraintType::Apart => format!(
                                "≥{}{} apart",
                                constraint.time_value,
                                if constraint.time_unit == crate::types::time_unit::TimeUnit::Hour {
                                    "h"
                                } else {
                                    "m"
                                }
                            ),
                        };

                        problem_constraints.push((entity_name.clone(), constraint_str));
                    }
                }
                Err(e) => {
                    problem_constraints.push((entity_name.clone(), format!("Error: {}", e)));
                }
            }
        }
    }

    if !problem_constraints.is_empty() {
        debug_error(compiler, "📋", "Problematic constraints found:");
        for (entity, constraint) in problem_constraints {
            debug_error(compiler, "  👉", &format!("{}: {}", entity, constraint));
        }
    } else {
        debug_error(compiler, "❓", "Could not identify specific problematic constraints. The combination of all constraints might be causing the issue.");
    }
}
