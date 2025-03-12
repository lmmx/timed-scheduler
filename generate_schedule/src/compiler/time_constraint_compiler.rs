use clock_zones::{Dbm, Zone};
use colored::*;
use std::collections::{HashMap, HashSet};
use std::env;

use crate::compiler::clock_info::ClockInfo;
use crate::compiler::constraints::{category, daily_bounds, entity, frequency};
use crate::compiler::debugging;
use crate::compiler::schedule_extraction;
use crate::extractor::schedule_extractor::ScheduleStrategy;
use crate::types::constraints::CategoryConstraint;
use crate::types::entity::Entity;

pub struct TimeConstraintCompiler {
    // Maps entity names to their data
    pub entities: HashMap<String, Entity>,
    // Maps category names to sets of entity names
    pub categories: HashMap<String, HashSet<String>>,
    // Maps clock IDs to their information
    pub clocks: HashMap<String, ClockInfo>,
    // The generated zone with constraints
    pub zone: Dbm<i64>,
    // Next available clock variable index
    pub next_clock_index: usize,
    // Debug mode flag
    pub debug: bool,
    // Optional category-level constraints
    pub category_constraints: Option<Vec<CategoryConstraint>>,
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
            category_constraints: None,
        }
    }

    // Add a setter method for category constraints
    pub fn set_category_constraints(&mut self, constraints: Vec<CategoryConstraint>) {
        self.category_constraints = Some(constraints);
    }

    fn allocate_clocks(&mut self) -> Result<(), String> {
        use clock_zones::Clock;

        for (entity_name, entity) in &self.entities {
            let instances = entity.frequency.get_instances_per_day();
            if self.debug {
                debugging::debug_print(
                    self,
                    "📝",
                    &format!(
                        "Entity: {} - Frequency: {:?} - Instances: {}",
                        entity_name, entity.frequency, instances
                    ),
                );
            }

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

                if self.debug {
                    debugging::debug_print(
                        self,
                        "➕",
                        &format!(
                            "Created clock: {} (var index: {})",
                            clock_id,
                            self.next_clock_index - 1
                        ),
                    );
                }
            }
        }

        Ok(())
    }

    pub fn compile(&mut self) -> Result<&Dbm<i64>, String> {
        debugging::debug_print(self, "🚀", "Starting compilation process");

        // 1. Create clock variables for all entity instances
        debugging::debug_print(self, "⏰", "Step 1: Allocating clock variables");
        self.allocate_clocks()?;
        debugging::debug_zone_state(self);

        // 2. Set daily bounds (0-24 hours in minutes)
        debugging::debug_print(self, "📅", "Step 2: Setting daily bounds (0-24 hours)");
        daily_bounds::apply_daily_bounds(self)?;
        debugging::debug_zone_state(self);

        // 3. Apply frequency-based constraints (spacing between occurrences)
        debugging::debug_print(self, "🔄", "Step 3: Applying frequency-based constraints");
        frequency::apply_frequency_constraints(self)?;
        debugging::debug_zone_state(self);

        // 4. Apply entity-specific constraints
        debugging::debug_print(self, "🔗", "Step 4: Applying entity-specific constraints");
        entity::apply_entity_constraints(self)?;
        debugging::debug_zone_state(self);

        // 5. Apply category-level constraints
        debugging::debug_print(self, "🔗", "Step 5: Applying category-level constraints");
        category::apply_category_constraints(self)?;
        debugging::debug_zone_state(self);

        // 6. Check feasibility
        if self.zone.is_empty() {
            debugging::debug_error(
                self,
                "❌",
                "Schedule is not feasible with the given constraints",
            );

            // Try to identify which constraint caused infeasibility
            debugging::debug_error(
                self,
                "🔍",
                "Attempting to identify problematic constraints...",
            );
            debugging::diagnose_infeasibility::<i32>(self);

            return Err("Schedule is not feasible with the given constraints".to_string());
        }

        debugging::debug_print(
            self,
            "✅",
            "Schedule is feasible! Zone has valid solutions.",
        );
        Ok(&self.zone)
    }

    pub fn find_clock_name(&self, var: clock_zones::Variable) -> Option<String> {
        for (name, info) in &self.clocks {
            if info.variable == var {
                return Some(name.clone());
            }
        }
        None
    }

    pub fn add_constraint_safely<F>(&mut self, constraint_builder: F, description: &str) -> bool
    where
        F: Fn() -> clock_zones::Constraint<i64>,
    {
        // Create a test zone to see if adding this constraint would make it infeasible
        let mut test_zone = self.zone.clone();
        test_zone.add_constraint(constraint_builder());

        if test_zone.is_empty() {
            debugging::debug_error(
                self,
                "⚠️",
                &format!(
                    "Cannot add constraint - would make schedule infeasible: {}",
                    description
                ),
            );
            false
        } else {
            debugging::debug_print(self, "✅", &format!("Adding constraint: {}", description));
            self.zone.add_constraint(constraint_builder());
            true
        }
    }

    pub fn finalize_schedule(
        &self,
        strategy: ScheduleStrategy,
    ) -> Result<HashMap<String, i32>, String> {
        use crate::extractor::schedule_extractor::ScheduleExtractor;

        // Make sure zone is properly compiled and feasible
        if self.zone.is_empty() {
            return Err(
                "Cannot extract schedule from empty zone. Did you call compile() first?"
                    .to_string(),
            );
        }

        // Create the extractor and pass references to zone and clocks
        let extractor = ScheduleExtractor::new(&self.zone, &self.clocks);

        // Extract schedule using the selected strategy
        let schedule = extractor.extract_schedule(strategy)?;

        // Debug output for schedule extraction
        if self.debug {
            println!(
                "{}",
                "📋 Schedule extracted using strategy:".yellow().bold()
            );
            match strategy {
                ScheduleStrategy::Earliest => println!("  Strategy: Earliest"),
                ScheduleStrategy::Latest => println!("  Strategy: Latest"),
                ScheduleStrategy::Centered => println!("  Strategy: Centered"),
                ScheduleStrategy::Justified => println!("  Strategy: Justified"),
                ScheduleStrategy::MaximumSpread => println!("  Strategy: MaximumSpread"),
            }

            // Convert to a sorted list like in format_schedule
            let mut time_entries: Vec<(i32, String)> = schedule
                .iter()
                .map(|(clock_id, &minutes)| (minutes, clock_id.clone()))
                .collect();

            // Sort by time ascending
            time_entries.sort_by_key(|&(minutes, _)| minutes);

            // Print the extracted times in sorted order
            for (minutes, clock_id) in time_entries {
                let hours = minutes / 60;
                let mins = minutes % 60;
                println!("  {}: {:02}:{:02}", clock_id.cyan(), hours, mins);
            }
            println!();
        }

        Ok(schedule)
    }

    // Delegate to schedule_extraction module
    pub fn extract_schedule(&self) -> Result<HashMap<String, i32>, String> {
        schedule_extraction::extract_schedule(self)
    }

    // Delegate to schedule_extraction module
    pub fn format_schedule(&self, schedule: &HashMap<String, i32>) -> String {
        schedule_extraction::format_schedule(self, schedule)
    }
}

pub fn try_disjunction<F1, F2>(
    &mut self,
    constraint1_builder: F1,
    constraint1_desc: &str,
    constraint2_builder: F2,
    constraint2_desc: &str,
) -> bool
where
    F1: Fn() -> clock_zones::Constraint<i64>,
    F2: Fn() -> clock_zones::Constraint<i64>,
{
    // Try first constraint
    let mut test_zone1 = self.zone.clone();
    test_zone1.add_constraint(constraint1_builder());
    let first_feasible = !test_zone1.is_empty();

    // Try second constraint
    let mut test_zone2 = self.zone.clone();
    test_zone2.add_constraint(constraint2_builder());
    let second_feasible = !test_zone2.is_empty();

    if !first_feasible && !second_feasible {
        // Neither constraint works
        debugging::debug_error(
            self,
            "⚠️",
            &format!(
                "Neither disjunctive constraint is feasible: {} OR {}",
                constraint1_desc, constraint2_desc
            ),
        );
        return false;
    } else if first_feasible && !second_feasible {
        // Only first constraint is feasible
        debugging::debug_print(
            self,
            "✅",
            &format!(
                "Choosing first disjunctive constraint (second is infeasible): {}",
                constraint1_desc
            ),
        );
        self.zone.add_constraint(constraint1_builder());
        return true;
    } else if !first_feasible && second_feasible {
        // Only second constraint is feasible
        debugging::debug_print(
            self,
            "✅",
            &format!(
                "Choosing second disjunctive constraint (first is infeasible): {}",
                constraint2_desc
            ),
        );
        self.zone.add_constraint(constraint2_builder());
        return true;
    } else {
        // Both constraints are feasible, choose the better one
        // For this implementation, let's use a simple heuristic:
        // Choose the constraint that results in a more balanced schedule

        // For a balanced schedule, we'll use a simple metric: compute the sum of
        // all shortest path differences between clocks after applying each constraint
        let mut sum1 = 0;
        let mut sum2 = 0;

        for i in 0..self.next_clock_index {
            for j in i + 1..self.next_clock_index {
                let var_i = clock_zones::Clock::variable(i);
                let var_j = clock_zones::Clock::variable(j);

                if let Some(diff1) = test_zone1.shortest_path(var_i, var_j) {
                    sum1 += diff1.abs();
                }

                if let Some(diff2) = test_zone2.shortest_path(var_i, var_j) {
                    sum2 += diff2.abs();
                }
            }
        }

        // Choose the constraint that results in smaller total differences,
        // which generally indicates a more balanced schedule
        if sum1 <= sum2 {
            debugging::debug_print(
                self,
                "✅",
                &format!(
                    "Both disjunctive constraints are feasible, choosing first based on schedule quality: {}",
                    constraint1_desc
                ),
            );
            self.zone.add_constraint(constraint1_builder());
        } else {
            debugging::debug_print(
                self,
                "✅",
                &format!(
                    "Both disjunctive constraints are feasible, choosing second based on schedule quality: {}",
                    constraint2_desc
                ),
            );
            self.zone.add_constraint(constraint2_builder());
        }
        return true;
    }
}
// Add this at the end of src/compiler/time_constraint_compiler.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::entity::Entity;
    use crate::types::frequency::FrequencyType;
    use clock_zones::{Clock, Constraint};

    #[test]
    fn test_try_disjunction() {
        // Create a simple compiler with two entities
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

        let mut compiler = TimeConstraintCompiler::new(vec![entity1, entity2]);

        // Allocate clocks and set daily bounds to initialize the zone
        compiler.allocate_clocks().unwrap();
        daily_bounds::apply_daily_bounds(&mut compiler).unwrap();

        // Get variables for testing
        let med1_var = compiler.clocks.get("medication_1").unwrap().variable;
        let meal1_var = compiler.clocks.get("meal_1").unwrap().variable;

        // Test case 1: Both constraints are feasible
        // Define two disjunctive constraints:
        // 1. Medication is 2h before meal
        let before_constraint = || Constraint::new_diff_ge(meal1_var, med1_var, 120);
        // 2. Medication is 1h after meal
        let after_constraint = || Constraint::new_diff_ge(med1_var, meal1_var, 60);

        // Try the disjunction
        let result = compiler.try_disjunction(
            before_constraint,
            "medication must be ≥2h before meal",
            after_constraint,
            "medication must be ≥1h after meal",
        );

        assert!(result, "Disjunction should be feasible");

        // Test case 2: Only the first constraint is feasible
        // Force the second constraint to be infeasible by adding a tight upper bound
        compiler
            .zone
            .add_constraint(Constraint::new_diff_le(med1_var, meal1_var, 30));

        // Define constraints again
        let before_constraint = || Constraint::new_diff_ge(meal1_var, med1_var, 120);
        let after_constraint = || Constraint::new_diff_ge(med1_var, meal1_var, 60);

        // Try the disjunction
        let result = compiler.try_disjunction(
            before_constraint,
            "medication must be ≥2h before meal",
            after_constraint,
            "medication must be ≥1h after meal",
        );

        assert!(result, "Disjunction should choose the feasible constraint");

        // Test case 3: Neither constraint is feasible
        // Reset the zone
        compiler = TimeConstraintCompiler::new(vec![entity1, entity2]);
        compiler.allocate_clocks().unwrap();
        daily_bounds::apply_daily_bounds(&mut compiler).unwrap();

        // Get variables again
        let med1_var = compiler.clocks.get("medication_1").unwrap().variable;
        let meal1_var = compiler.clocks.get("meal_1").unwrap().variable;

        // Force a tight schedule where neither constraint can be satisfied
        compiler
            .zone
            .add_constraint(Constraint::new_diff_le(med1_var, meal1_var, 30)); // Med before meal by at most 30 min
        compiler
            .zone
            .add_constraint(Constraint::new_diff_le(meal1_var, med1_var, 30)); // Meal before med by at most 30 min

        // Define constraints again
        let before_constraint = || Constraint::new_diff_ge(meal1_var, med1_var, 120);
        let after_constraint = || Constraint::new_diff_ge(med1_var, meal1_var, 60);

        // Try the disjunction
        let result = compiler.try_disjunction(
            before_constraint,
            "medication must be ≥2h before meal",
            after_constraint,
            "medication must be ≥1h after meal",
        );

        assert!(
            !result,
            "Disjunction should fail when neither constraint is feasible"
        );
    }
}
