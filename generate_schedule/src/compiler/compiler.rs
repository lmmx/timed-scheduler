use clock_zones::{Bound, Clock, Constraint, Dbm, Variable, Zone};
use colored::*;
use std::collections::{HashMap, HashSet};
use std::env;

use crate::compiler::clock_info::ClockInfo;
use crate::types::constraints::{ConstraintExpression, ConstraintReference, ConstraintType};
use crate::types::entity::Entity;
use crate::types::frequency::Frequency;
use crate::types::time_unit::TimeUnit::Hour;

pub struct TimeConstraintCompiler {
    // Maps entity names to their data
    entities: HashMap<String, Entity>,
    // Maps category names to sets of entity names
    categories: HashMap<String, HashSet<String>>,
    // Maps clock IDs to their information
    clocks: HashMap<String, ClockInfo>,
    // The generated zone with constraints
    zone: Dbm<i64>,
    // Next available clock variable index
    next_clock_index: usize,
    // Debug mode flag
    debug: bool,
}

impl TimeConstraintCompiler {
    pub fn new(entities: Vec<Entity>) -> Self {
        // Check if debug flag is set
        let debug = env::var("RUST_DEBUG").is_ok() || env::args().any(|arg| arg == "--debug");

        // Organize entities and categories
        let mut entity_map = HashMap::new();
        let mut category_map: HashMap<String, HashSet<String>> = HashMap::new();

        for entity in entities {
            // Add to category map
            category_map
                .entry(entity.category.clone())
                .or_default()
                .insert(entity.name.clone());

            // Add to entity map
            entity_map.insert(entity.name.clone(), entity);
        }

        // Calculate total clock variables needed
        let total_clocks = entity_map
            .values()
            .map(|e| e.frequency.get_instances_per_day())
            .sum();

        let zone = Dbm::new_unconstrained(total_clocks);

        TimeConstraintCompiler {
            entities: entity_map,
            categories: category_map,
            clocks: HashMap::new(),
            zone,
            next_clock_index: 0,
            debug,
        }
    }

    fn debug_print(&self, emoji: &str, message: &str) {
        if self.debug {
            println!("{} {}", emoji.green(), message.bright_blue());
        }
    }

    fn debug_error(&self, emoji: &str, message: &str) {
        if self.debug {
            println!("{} {}", emoji.red(), message.bright_red());
        }
    }

    fn debug_zone_state(&self) {
        if !self.debug {
            return;
        }

        println!("{}", "🔍 Current Zone State:".yellow().bold());

        if self.zone.is_empty() {
            println!("{}", "   ❌ ZONE IS EMPTY (infeasible)".red().bold());
            return;
        }

        println!("{}", "   ✅ ZONE IS FEASIBLE".green().bold());

        // Print bounds for each clock
        for (clock_id, clock_info) in &self.clocks {
            let lower = self.zone.get_lower_bound(clock_info.variable);
            let upper = self.zone.get_upper_bound(clock_info.variable);

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

        for i in 0..self.clocks.len() {
            for j in 0..self.clocks.len() {
                if i == j {
                    continue;
                }

                let var_i = Clock::variable(i);
                let var_j = Clock::variable(j);

                let bound = self.zone.get_bound(var_i, var_j);
                if let Some(diff) = bound.constant() {
                    if constraints_shown < 5 {
                        // Limit to 5 constraints to avoid overwhelming output
                        let name_i = self
                            .find_clock_name(var_i)
                            .unwrap_or_else(|| "unknown".to_string());
                        let name_j = self
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

    fn find_clock_name(&self, var: Variable) -> Option<String> {
        for (name, info) in &self.clocks {
            if info.variable == var {
                return Some(name.clone());
            }
        }
        None
    }

    pub fn compile(&mut self) -> Result<&Dbm<i64>, String> {
        self.debug_print("🚀", "Starting compilation process");

        // 1. Create clock variables for all entity instances
        self.debug_print("⏰", "Step 1: Allocating clock variables");
        self.allocate_clocks()?;
        self.debug_zone_state();

        // 2. Set daily bounds (0-24 hours in minutes)
        self.debug_print("📅", "Step 2: Setting daily bounds (0-24 hours)");
        self.set_daily_bounds()?;
        self.debug_zone_state();

        // 3. Apply frequency-based constraints (spacing between occurrences)
        self.debug_print("🔄", "Step 3: Applying frequency-based constraints");
        self.apply_frequency_constraints()?;
        self.debug_zone_state();

        // 4. Apply entity-specific constraints
        self.debug_print("🔗", "Step 4: Applying entity-specific constraints");
        self.apply_entity_constraints()?;
        self.debug_zone_state();

        // 5. Check feasibility
        if self.zone.is_empty() {
            self.debug_error("❌", "Schedule is not feasible with the given constraints");

            // Try to identify which constraint caused infeasibility
            self.debug_error("🔍", "Attempting to identify problematic constraints...");
            self.diagnose_infeasibility::<i32>();

            return Err("Schedule is not feasible with the given constraints".to_string());
        }

        self.debug_print("✅", "Schedule is feasible! Zone has valid solutions.");
        Ok(&self.zone)
    }

    fn diagnose_infeasibility<B: clock_zones::Bound<Constant = i32>>(&mut self) {
        if !self.debug {
            return;
        }

        self.debug_print("🔎", "Running diagnosis to find problematic constraints");

        // Try with just daily bounds
        let mut test_zone = Dbm::<B>::new_zero(self.next_clock_index);

        // Apply only daily bounds
        for clock_info in self.clocks.values() {
            test_zone.add_constraint(Constraint::new_ge(clock_info.variable, 0));
            test_zone.add_constraint(Constraint::new_le(clock_info.variable, 1439));
        }

        if test_zone.is_empty() {
            self.debug_error(
                "⚠️",
                "Even basic daily bounds (0-1439) lead to infeasibility!",
            );
            return;
        }

        self.debug_print("✓", "Basic daily bounds are feasible");

        // Try with frequency constraints
        let mut test_zone = Dbm::new_zero(self.next_clock_index);

        // Apply daily bounds
        for clock_info in self.clocks.values() {
            test_zone.add_constraint(Constraint::new_ge(clock_info.variable, 0));
            test_zone.add_constraint(Constraint::new_le(clock_info.variable, 1439));
        }

        // Group clocks by entity
        let mut entity_clocks: HashMap<String, Vec<Variable>> = HashMap::new();
        for clock_info in self.clocks.values() {
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

            let mut ordered_clocks: Vec<(usize, Variable)> = self
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
            self.debug_error("⚠️", "Ordering constraints lead to infeasibility!");
            return;
        }

        self.debug_print("✓", "Basic ordering constraints are feasible");

        // Now try applying spacing constraints
        for (entity_name, clocks) in &entity_clocks {
            if clocks.len() <= 1 {
                continue;
            }

            let entity = self.entities.get(entity_name).unwrap();

            let mut ordered_clocks: Vec<(usize, Variable)> = self
                .clocks
                .values()
                .filter(|c| c.entity_name == *entity_name)
                .map(|c| (c.instance, c.variable))
                .collect();
            ordered_clocks.sort_by_key(|&(instance, _)| instance);

            let mut test_zone_with_spacing = test_zone.clone();
            let min_spacing = match entity.frequency {
                Frequency::TwiceDaily => 6 * 60,      // 6 hours in minutes
                Frequency::ThreeTimesDaily => 4 * 60, // 4 hours in minutes
                Frequency::EveryXHours(hours) => (hours as i64) * 60,
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
                self.debug_error(
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

        for (entity_name, entity) in &self.entities {
            for constraint in &entity.constraints {
                let mut test_zone_with_constraint = test_zone.clone();

                match self.apply_test_constraint(
                    &mut test_zone_with_constraint,
                    entity_name,
                    constraint,
                ) {
                    Ok(_) => {
                        if test_zone_with_constraint.is_empty() {
                            let constraint_str = match &constraint.constraint_type {
                                ConstraintType::Before => format!(
                                    "≥{}{} before {:?}",
                                    constraint.time_value,
                                    if constraint.time_unit == Hour {
                                        "h"
                                    } else {
                                        "m"
                                    },
                                    constraint.reference
                                ),
                                ConstraintType::After => format!(
                                    "≥{}{} after {:?}",
                                    constraint.time_value,
                                    if constraint.time_unit == Hour {
                                        "h"
                                    } else {
                                        "m"
                                    },
                                    constraint.reference
                                ),
                                ConstraintType::ApartFrom => format!(
                                    "≥{}{} apart from {:?}",
                                    constraint.time_value,
                                    if constraint.time_unit == Hour {
                                        "h"
                                    } else {
                                        "m"
                                    },
                                    constraint.reference
                                ),
                                ConstraintType::Apart => format!(
                                    "≥{}{} apart",
                                    constraint.time_value,
                                    if constraint.time_unit == Hour {
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
            self.debug_error("📋", "Problematic constraints found:");
            for (entity, constraint) in problem_constraints {
                self.debug_error("  👉", &format!("{}: {}", entity, constraint));
            }
        } else {
            self.debug_error("❓", "Could not identify specific problematic constraints. The combination of all constraints might be causing the issue.");
        }
    }

    fn apply_test_constraint(
        &self,
        test_zone: &mut Dbm<i64>,
        entity_name: &str,
        constraint: &ConstraintExpression,
    ) -> Result<(), String> {
        // Convert time value to minutes
        let time_in_minutes = constraint.time_unit.to_minutes(constraint.time_value) as i64;

        // Get all clocks for this entity
        let entity_clocks: Vec<Variable> = self
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
                        self.resolve_reference(reference_str)?
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

    fn allocate_clocks(&mut self) -> Result<(), String> {
        for (entity_name, entity) in &self.entities {
            let instances = entity.frequency.get_instances_per_day();
            self.debug_print(
                "📝",
                &format!(
                    "Entity: {} - Frequency: {:?} - Instances: {}",
                    entity_name, entity.frequency, instances
                ),
            );

            for i in 0..instances {
                let clock_id = format!("{}_{}", entity_name, i + 1);
                let variable = Clock::variable(self.next_clock_index);
                self.next_clock_index += 1;

                self.clocks.insert(
                    clock_id.clone(),
                    ClockInfo {
                        entity_name: entity_name.clone(),
                        instance: i + 1,
                        variable,
                    },
                );

                self.debug_print(
                    "➕",
                    &format!(
                        "Created clock: {} (var index: {})",
                        clock_id,
                        self.next_clock_index - 1
                    ),
                );
            }
        }

        Ok(())
    }

    fn set_daily_bounds(&mut self) -> Result<(), String> {
        // Convert time to minutes (0-1440 for a 24-hour day)
        for (clock_id, clock_info) in &self.clocks {
            // Not before 0:00
            self.zone
                .add_constraint(Constraint::new_ge(clock_info.variable, 0));
            // Not after 23:59
            self.zone
                .add_constraint(Constraint::new_le(clock_info.variable, 1439));

            self.debug_print("⏱️", &format!("Set bounds for {}: [0:00, 23:59]", clock_id));
        }

        Ok(())
    }

    fn apply_frequency_constraints(&mut self) -> Result<(), String> {
        // Group clocks by entity
        let mut entity_clocks: HashMap<String, Vec<Variable>> = HashMap::new();

        for clock_info in self.clocks.values() {
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

            let entity = self.entities.get(&entity_name).unwrap();

            // Sort clocks by instance number
            let mut ordered_clocks: Vec<(usize, Variable)> = self
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
                self.zone
                    .add_constraint(Constraint::new_diff_gt(next, current, 0));

                self.debug_print(
                    "🔢",
                    &format!(
                        "{}_{}  must be after  {}_{}",
                        entity_name, instance_j, entity_name, instance_i
                    ),
                );

                // Apply minimum spacing only if specified
                let min_spacing = if let Some(spacing) = entity.min_spacing {
                    spacing as i64
                } else {
                    0 // no default enforced spacing
                };

                self.zone
                    .add_constraint(Constraint::new_diff_ge(next, current, min_spacing));

                let hours = min_spacing / 60;
                let mins = min_spacing % 60;
                self.debug_print(
                    "↔️",
                    &format!(
                        "{}_{}  must be ≥{}h{}m after  {}_{}",
                        entity_name, instance_j, hours, mins, entity_name, instance_i
                    ),
                );
            }
        }

        Ok(())
    }

    fn apply_entity_constraints(&mut self) -> Result<(), String> {
        // Collect all constraints first to avoid borrowing issues
        let mut all_constraints: Vec<(String, ConstraintExpression)> = Vec::new();

        for (entity_name, entity) in &self.entities {
            for constraint in &entity.constraints {
                all_constraints.push((entity_name.clone(), constraint.clone()));
            }
        }

        // Now apply all collected constraints
        for (entity_name, constraint) in all_constraints {
            self.apply_constraint(&entity_name, &constraint)?;
        }

        Ok(())
    }

    fn add_constraint_safely<F>(&mut self, constraint_builder: F, description: &str) -> bool
    where
        F: Fn() -> Constraint<i64>,
    {
        // Create a test zone to see if adding this constraint would make it infeasible
        let mut test_zone = self.zone.clone();
        test_zone.add_constraint(constraint_builder());

        if test_zone.is_empty() {
            self.debug_error(
                "⚠️",
                &format!(
                    "Cannot add constraint - would make schedule infeasible: {}",
                    description
                ),
            );
            false
        } else {
            self.debug_print("✅", &format!("Adding constraint: {}", description));
            self.zone.add_constraint(constraint_builder());
            true
        }
    }

    fn apply_constraint(
        &mut self,
        entity_name: &str,
        constraint: &ConstraintExpression,
    ) -> Result<(), String> {
        // Convert time value to minutes
        let time_in_minutes = constraint.time_unit.to_minutes(constraint.time_value) as i64;

        // Get all clocks for this entity
        let entity_clocks: Vec<Variable> = self
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
                        // Get clock names for better logging
                        let name_i = self
                            .find_clock_name(entity_clocks[i])
                            .unwrap_or_else(|| format!("clock_{}", i));
                        let name_j = self
                            .find_clock_name(entity_clocks[j])
                            .unwrap_or_else(|| format!("clock_{}", j));

                        // Ensure minimum spacing from i to j
                        let hours = time_in_minutes / 60;
                        let mins = time_in_minutes % 60;
                        let description_i_to_j = format!(
                            "{} must be ≥{}h{}m apart from {} (forward)",
                            name_i, hours, mins, name_j
                        );
                        let success_i_to_j = self.add_constraint_safely(
                            || {
                                Constraint::new_diff_ge(
                                    entity_clocks[i],
                                    entity_clocks[j],
                                    time_in_minutes,
                                )
                            },
                            &description_i_to_j,
                        );

                        // Only try the other direction if the first one was successful
                        if success_i_to_j {
                            // Ensure minimum spacing from j to i
                            let description_j_to_i = format!(
                                "{} must be ≥{}h{}m apart from {} (backward)",
                                name_j, hours, mins, name_i
                            );

                            self.add_constraint_safely(
                                || {
                                    Constraint::new_diff_ge(
                                        entity_clocks[j],
                                        entity_clocks[i],
                                        time_in_minutes,
                                    )
                                },
                                &description_j_to_i,
                            );
                        }
                    }
                }
            }

            ConstraintType::Before | ConstraintType::After | ConstraintType::ApartFrom => {
                // Get reference clocks based on the constraint reference
                let reference_clocks = match &constraint.reference {
                    ConstraintReference::Unresolved(reference_str) => {
                        self.resolve_reference(reference_str)?
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
                                self.zone.add_constraint(Constraint::new_diff_ge(
                                    reference_clock,
                                    entity_clock,
                                    time_in_minutes,
                                ));
                            }
                            ConstraintType::After => {
                                // Entity must be scheduled at least X minutes after reference
                                self.zone.add_constraint(Constraint::new_diff_ge(
                                    entity_clock,
                                    reference_clock,
                                    time_in_minutes,
                                ));
                            }
                            ConstraintType::ApartFrom => {
                                // Entity must be separated from reference by at least X minutes
                                // in either direction
                                self.zone.add_constraint(Constraint::new_diff_ge(
                                    entity_clock,
                                    reference_clock,
                                    time_in_minutes,
                                ));
                                self.zone.add_constraint(Constraint::new_diff_ge(
                                    reference_clock,
                                    entity_clock,
                                    time_in_minutes,
                                ));
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // Extract a concrete schedule from the zone
    pub fn extract_schedule(&self) -> Result<HashMap<String, i32>, String> {
        if self.zone.is_empty() {
            return Err("Cannot extract schedule from empty zone".to_string());
        }

        let mut schedule = HashMap::new();

        // For each clock, get a feasible time
        for (clock_id, clock_info) in &self.clocks {
            // Get the lower and upper bounds for this clock (in minutes)
            let lower = self.zone.get_lower_bound(clock_info.variable).unwrap_or(0);
            let upper = self
                .zone
                .get_upper_bound(clock_info.variable)
                .unwrap_or(1439);

            // Choose a time in the middle of the feasible range
            let time_in_minutes = ((lower + upper) / 2) as i32;
            schedule.insert(clock_id.clone(), time_in_minutes);
        }

        Ok(schedule)
    }

    // Format the schedule into a human-readable format
    pub fn format_schedule(&self, schedule: &HashMap<String, i32>) -> String {
        let mut result = String::new();
        result.push_str("Daily Schedule:\n");

        // Convert minutes to HH:MM format and sort by time
        let mut time_entries: Vec<(String, String)> = schedule
            .iter()
            .map(|(clock_id, &minutes)| {
                let hours = minutes / 60;
                let mins = minutes % 60;
                let time_str = format!("{:02}:{:02}", hours, mins);
                (time_str.clone(), format!("{}: {}", clock_id, time_str))
            })
            .collect();

        // Sort by time
        time_entries.sort_by(|a, b| a.0.cmp(&b.0));

        // Add to result
        for (_, entry) in time_entries {
            result.push_str(&format!("  {}\n", entry));
        }

        // Group by entity
        result.push_str("\nBy Entity:\n");

        let mut entity_schedules: HashMap<String, Vec<(String, i32)>> = HashMap::new();

        for (clock_id, &minutes) in schedule {
            if let Some(clock_info) = self.clocks.get(clock_id) {
                entity_schedules
                    .entry(clock_info.entity_name.clone())
                    .or_default()
                    .push((clock_id.clone(), minutes));
            }
        }

        // Sort entities alphabetically
        let mut entity_names: Vec<String> = entity_schedules.keys().cloned().collect();
        entity_names.sort();

        for entity_name in entity_names {
            let entity = self.entities.get(&entity_name).unwrap();
            result.push_str(&format!("  {} ({}):\n", entity_name, entity.category));

            let times = entity_schedules.get(&entity_name).unwrap();
            let mut sorted_times = times.clone();
            sorted_times.sort_by_key(|&(_, minutes)| minutes);

            for (clock_id, minutes) in sorted_times {
                let hours = minutes / 60;
                let mins = minutes % 60;
                result.push_str(&format!("    {}: {:02}:{:02}", clock_id, hours, mins));

                // Add amount information if available
                if let Some(entity) = self.entities.get(&entity_name) {
                    if let Some(amount) = entity.amount {
                        if let Some(split) = entity.split {
                            // If we have both amount and split
                            let per_instance = amount / split as f64;
                            result.push_str(&format!(" - {:.1} {}", per_instance, entity.unit));
                        } else {
                            // If we have just amount
                            result.push_str(&format!(" - {:.1} {}", amount, entity.unit));
                        }
                    } else if let Some(split) = entity.split {
                        // If we have just split
                        result.push_str(&format!(" - 1/{} {}", split, entity.unit));
                    }
                }

                result.push_str("\n");
            }
        }

        result
    }

    // Enhanced resolve_reference method to handle "or" expressions without sorting
    fn resolve_reference(&self, reference_str: &str) -> Result<Vec<Variable>, String> {
        // Check if the reference contains " or "
        if reference_str.contains(" or ") {
            let parts: Vec<&str> = reference_str.split(" or ").collect();
            let mut all_clocks = Vec::new();

            // Resolve each part separately and combine the results
            for part in parts {
                match self.resolve_single_reference(part.trim()) {
                    Ok(clocks) => {
                        // Add only unique clocks
                        for clock in clocks {
                            if !all_clocks.contains(&clock) {
                                all_clocks.push(clock);
                            }
                        }
                    }
                    Err(_) => (), // Ignore errors for individual parts in an OR expression
                }
            }

            if all_clocks.is_empty() {
                return Err(format!(
                    "Could not resolve any part of reference '{}'",
                    reference_str
                ));
            }

            return Ok(all_clocks);
        }

        // If no "or", just resolve as a single reference
        self.resolve_single_reference(reference_str)
    }

    // Helper method to resolve a single reference (no OR)
    fn resolve_single_reference(&self, reference_str: &str) -> Result<Vec<Variable>, String> {
        // First try to find it as an entity (exact match)
        let entity_clocks: Vec<Variable> = self
            .clocks
            .values()
            .filter(|c| c.entity_name.to_lowercase() == reference_str)
            .map(|c| c.variable)
            .collect();

        if !entity_clocks.is_empty() {
            return Ok(entity_clocks);
        }

        // If not found as entity, try as a category
        if let Some(entities) = self.categories.get(reference_str) {
            let category_clocks: Vec<Variable> = self
                .clocks
                .values()
                .filter(|c| entities.contains(&c.entity_name))
                .map(|c| c.variable)
                .collect();

            if !category_clocks.is_empty() {
                return Ok(category_clocks);
            }
        }

        // If still not found, return an error
        Err(format!(
            "Could not resolve reference '{}' - not found as entity or category",
            reference_str
        ))
    }
}
