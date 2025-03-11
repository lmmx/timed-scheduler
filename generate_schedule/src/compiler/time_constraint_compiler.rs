use clock_zones::{Dbm, Zone};
use std::collections::{HashMap, HashSet};
use std::env;

use crate::compiler::clock_info::ClockInfo;
use crate::compiler::constraints::{
    daily_bounds::apply_daily_bounds, entity::apply_entity_constraints,
    frequency::apply_frequency_constraints,
};
use crate::compiler::debugging::diagnose_infeasibility;
use crate::compiler::reference_resolution::resolve_reference;
use crate::compiler::schedule_extraction::{extract_schedule, format_schedule};
use crate::extractor::schedule_extractor::ScheduleStrategy;
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

    fn allocate_clocks(&mut self) -> Result<(), String> {
        use clock_zones::Clock;

        for (entity_name, entity) in &self.entities {
            let instances = entity.frequency.get_instances_per_day();
            if self.debug {
                use crate::compiler::debugging::debug_print;
                debug_print(
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
                    use crate::compiler::debugging::debug_print;
                    debug_print(
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
        use crate::compiler::debugging::{debug_error, debug_print, debug_zone_state};

        debug_print(self, "🚀", "Starting compilation process");

        // 1. Create clock variables for all entity instances
        debug_print(self, "⏰", "Step 1: Allocating clock variables");
        self.allocate_clocks()?;
        debug_zone_state(self);

        // 2. Set daily bounds (0-24 hours in minutes)
        debug_print(self, "📅", "Step 2: Setting daily bounds (0-24 hours)");
        apply_daily_bounds(self)?;
        debug_zone_state(self);

        // 3. Apply frequency-based constraints (spacing between occurrences)
        debug_print(self, "🔄", "Step 3: Applying frequency-based constraints");
        apply_frequency_constraints(self)?;
        debug_zone_state(self);

        // 4. Apply entity-specific constraints
        debug_print(self, "🔗", "Step 4: Applying entity-specific constraints");
        apply_entity_constraints(self)?;
        debug_zone_state(self);

        // 5. Check feasibility
        if self.zone.is_empty() {
            debug_error(
                self,
                "❌",
                "Schedule is not feasible with the given constraints",
            );

            // Try to identify which constraint caused infeasibility
            debug_error(
                self,
                "🔍",
               "Attempting to identify problematic constraints..."),
            ;
            diagnose_infeasibility::<i32>(self);

            return Err("Schedule is not feasible with the given constraints".to_string());
        }

        debug_print(
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
        use crate::compiler::debugging::{debug_error, debug_print};

        // Create a test zone to see if adding this constraint would make it infeasible
        let mut test_zone = self.zone.clone();
        test_zone.add_constraint(constraint_builder());

        if test_zone.is_empty() {
            debug_error(
                self,
                "⚠️",
                &format!(
                    "Cannot add constraint - would make schedule infeasible: {}",
                    description
                ),
            );
            false
        } else {
            debug_print(self, "✅", &format!("Adding constraint: {}", description));
            self.zone.add_constraint(constraint_builder());
            true
        }
    }

    pub fn finalize_schedule(
        &self,
        strategy: ScheduleStrategy,
    ) -> Result<HashMap<String, i32>, String> {
        use crate::extractor::schedule_extractor::ScheduleExtractor;
        use colored::*;

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
}
