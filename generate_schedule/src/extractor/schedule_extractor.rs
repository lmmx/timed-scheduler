use clock_zones::{AnyClock, Bound, Dbm, Zone};
use std::collections::HashMap;
use colored::*; // Add colored crate for consistent styling with compiler
use std::env;

use crate::compiler::clock_info::ClockInfo;

#[derive(Debug, Clone, Copy)]
pub enum ScheduleStrategy {
    Earliest,
    Latest,
    Centered,
    Justified,
    MaximumSpread,
}

/// A small struct to hold lower/upper bounds for a clock.
struct Bounds {
    lb: i64,
    ub: i64,
}

pub struct ScheduleExtractor<'a> {
    pub zone: &'a Dbm<i64>,
    pub clocks: &'a HashMap<String, ClockInfo>,
    debug: bool,
}

impl<'a> ScheduleExtractor<'a> {
    pub fn new(zone: &'a Dbm<i64>, clocks: &'a HashMap<String, ClockInfo>) -> Self {
        // Check if debug flag is set - same approach as compiler
        let debug = env::var("RUST_DEBUG").is_ok() || env::args().any(|arg| arg == "--debug");
        
        Self { zone, clocks, debug }
    }

    // New debug methods to match the compiler's style
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

    fn debug_bounds(&self, clock_id: &str, bounds: &Bounds) {
        if self.debug {
            let lb_hour = bounds.lb / 60;
            let lb_min = bounds.lb % 60;
            let ub_hour = bounds.ub / 60;
            let ub_min = bounds.ub % 60;
            
            println!("   {} bounds: [{:02}:{:02} - {:02}:{:02}]", 
                clock_id.cyan(),
                lb_hour, lb_min,
                ub_hour, ub_min
            );
        }
    }

    fn debug_set_time(&self, clock_id: &str, time: i32) {
        if self.debug {
            let hours = time / 60;
            let mins = time % 60;
            println!("   Set {} to {:02}:{:02}", clock_id.cyan(), hours, mins);
        }
    }

    fn get_bounds(&self, variable: impl AnyClock) -> Bounds {
        let lb = self.zone.get_lower_bound(variable).unwrap_or(0);
        let ub = self.zone.get_upper_bound(variable).unwrap_or(1440);
        Bounds { lb, ub }
    }

    fn is_within_bounds(&self, variable: impl AnyClock, time: i32) -> bool {
        let bounds = self.get_bounds(variable);
        let result = time >= bounds.lb as i32 && time <= bounds.ub as i32;
        if !result && self.debug {
            self.debug_error("⚠️", &format!(
                "Time {} is outside bounds [{}, {}]", 
                time, bounds.lb, bounds.ub
            ));
        }
        result
    }
    
    fn clamp_to_bounds(&self, variable: impl AnyClock, time: i32) -> i32 {
        let bounds = self.get_bounds(variable);
        let clamped = time.clamp(bounds.lb as i32, bounds.ub as i32);
        if clamped != time && self.debug {
            self.debug_print("🔄", &format!(
                "Clamped time {} to {} (bounds: [{}, {}])", 
                time, clamped, bounds.lb, bounds.ub
            ));
        }
        clamped
    }
    
    // Check and enforce a difference constraint between two clocks
    // Returns true if the schedule was changed
    fn enforce_constraint(
        &self,
        schedule: &mut HashMap<String, i32>,
        first_id: &str,
        first_var: impl AnyClock + Copy,
        first_time: i32,
        second_id: &str,
        second_var: impl AnyClock + Copy,
        second_time: i32,
    ) -> Result<bool, String> {
        // Check if there's a difference constraint: second - first <= bound
        if let Some(bound) = self.zone.get_bound(first_var, second_var).constant() {
            // Check if our current schedule violates this constraint
            if (second_time - first_time) > bound as i32 {
                self.debug_error("⚠️", &format!(
                    "Constraint violation: {} - {} = {} > {}", 
                    second_id, first_id, second_time - first_time, bound
                ));
                
                // Try adjusting the second clock first
                let new_second_time = first_time + bound as i32;
                let second_bounds = self.get_bounds(second_var);
                let second_ub = second_bounds.ub as i32;
    
                // Make sure the new time is within second clock's upper bound
                if new_second_time <= second_ub {
                    self.debug_print("🔧", &format!(
                        "Adjusting {} from {} to {} to satisfy constraint",
                        second_id, second_time, new_second_time
                    ));
                    schedule.insert(second_id.to_string(), new_second_time);
                    return Ok(true);
                }
                
                self.debug_print("❌", &format!(
                    "Cannot adjust {} down to {} (upper bound: {})",
                    second_id, new_second_time, second_ub
                ));
                
                // If we can't move second clock down enough, try moving first clock up
                let new_first_time = second_time - bound as i32;
                let first_bounds = self.get_bounds(first_var);
                let first_lb = first_bounds.lb as i32;
    
                if new_first_time >= first_lb {
                    self.debug_print("🔧", &format!(
                        "Adjusting {} from {} to {} to satisfy constraint",
                        first_id, first_time, new_first_time
                    ));
                    schedule.insert(first_id.to_string(), new_first_time);
                    return Ok(true);
                }
                
                self.debug_error("❌", &format!(
                    "Cannot adjust {} up to {} (lower bound: {})",
                    first_id, new_first_time, first_lb
                ));
                
                // If neither adjustment works, report the conflict
                return Err(format!(
                    "Cannot satisfy constraint: {} - {} <= {} in relaxation",
                    second_id, first_id, bound
                ));
            }
        }
        
        Ok(false)
    }

    fn extract_with_strategy<F>(&self, time_selector: F) -> Result<HashMap<String, i32>, String>
    where
        F: Fn(i64, i64) -> i64,
    {
        self.debug_print("🚀", "Extracting schedule with custom strategy");
        
        let mut schedule = HashMap::new();
        for (clock_id, info) in self.clocks.iter() {
            let bounds = self.get_bounds(info.variable);
            self.debug_bounds(clock_id, &bounds);
            
            let time = time_selector(bounds.lb, bounds.ub);
            self.debug_set_time(clock_id, time as i32);
            
            schedule.insert(clock_id.clone(), time as i32);
        }
        Ok(schedule)
    }

    pub fn extract_schedule(
        &self,
        strategy: ScheduleStrategy,
    ) -> Result<HashMap<String, i32>, String> {
        self.debug_print("🧩", &format!("Extracting schedule with {:?} strategy", strategy));
        
        // Feasibility check
        if self.zone.is_empty() {
            self.debug_error("❌", "Zone is empty; no schedule is possible.");
            return Err("Zone is empty; no schedule is possible.".to_string());
        }
    
        // Dispatch to appropriate strategy
        let mut schedule = match strategy {
            ScheduleStrategy::Earliest => {
                self.debug_print("⏱️", "Using Earliest strategy - placing all events at their earliest possible times");
                self.extract_earliest()
            },
            ScheduleStrategy::Latest => {
                self.debug_print("⏰", "Using Latest strategy - placing all events at their latest possible times");
                self.extract_latest()
            },
            ScheduleStrategy::Centered => {
                self.debug_print("⚖️", "Using Centered strategy - placing all events at the middle of their feasible ranges");
                self.extract_centered()
            },
            ScheduleStrategy::Justified => {
                self.debug_print("📏", "Using Justified strategy - distributing events to span the entire feasible range");
                self.extract_justified_global()
            },
            ScheduleStrategy::MaximumSpread => {
                self.debug_print("↔️", "Using MaximumSpread strategy - maximizing distance between consecutive events");
                self.extract_max_spread_global()
            },
        }?;
    
        // Final validation to ensure all times are within bounds
        self.debug_print("✅", "Validating final schedule");
        self.validate_schedule(&mut schedule)?;
    
        self.debug_print("🏁", "Schedule extraction complete");
        Ok(schedule)
    }

    // Ensure all clock assignments are within their bounds
    fn validate_schedule(&self, schedule: &mut HashMap<String, i32>) -> Result<(), String> {
        self.debug_print("🔎", "Validating schedule - checking all times are within bounds");
        
        for (clock_id, info) in self.clocks.iter() {
            if let Some(time) = schedule.get_mut(clock_id) {
                if !self.is_within_bounds(info.variable, *time) {
                    let old_time = *time;
                    // Clamp to valid range
                    *time = self.clamp_to_bounds(info.variable, *time);
                    self.debug_print("📌", &format!(
                        "Adjusted {} from {} to {} to fit within bounds", 
                        clock_id, old_time, *time
                    ));
                }
            }
        }
        Ok(())
    }

    // Sort clocks topologically by entity name and instance number
    fn sort_clocks_topologically(&self) -> Vec<(String, i64, i64, usize, String)> {
        self.debug_print("🔄", "Sorting clocks topologically");
        
        let mut all_vars: Vec<(String, i64, i64, usize, String)> = Vec::new();
        
        // Collect all clocks with their bounds
        for (clock_id, info) in &*self.clocks {
            let bounds = self.get_bounds(info.variable);
            all_vars.push((
                clock_id.clone(),
                bounds.lb,
                bounds.ub,
                info.instance,
                info.entity_name.clone(),
            ));
            
            if self.debug {
                self.debug_bounds(clock_id, &bounds);
            }
        }
        
        // Sort first by entity name, then by instance number
        all_vars.sort_by(
            |(_, _, _, instance_a, entity_a), (_, _, _, instance_b, entity_b)| {
                // First sort by entity name
                let entity_cmp = entity_a.cmp(entity_b);
                if entity_cmp != std::cmp::Ordering::Equal {
                    return entity_cmp;
                }
                // Then by instance number if same entity
                instance_a.cmp(instance_b)
            },
        );
        
        if self.debug {
            self.debug_print("📋", "Sorted clock order:");
            for (i, (id, _, _, instance, entity)) in all_vars.iter().enumerate() {
                println!("   {}. {} ({}, instance {})", i+1, id.cyan(), entity.blue(), instance);
            }
        }
        
        all_vars
    }

    fn prepare_global_schedule(&self) -> Result<(Vec<(String, i64, i64, usize, String)>, i64, i64), String> {
        self.debug_print("🔍", "Preparing global schedule");
        
        // Collect all clocks with their bounds
        let all_vars = self.sort_clocks_topologically();
        
        if all_vars.is_empty() {
            self.debug_error("❌", "No clocks found to schedule");
            return Err("No clocks found to schedule".to_string());
        }
    
        // Find the feasible span for the entire schedule
        let global_min = 0;
        let global_max = 1440;
    
        self.debug_print("📊", &format!(
            "Global schedule span: [{:02}:{:02} - {:02}:{:02}]",
            global_min / 60, global_min % 60,
            global_max / 60, global_max % 60
        ));
    
        // Safety check: ensure we have a valid span
        if global_min >= global_max {
            self.debug_error("❌", &format!(
                "No valid global span available: min={}, max={}", 
                global_min, global_max
            ));
            return Err(format!(
                "No valid global span available: min={}, max={}", 
                global_min, global_max
            ));
        }
    
        Ok((all_vars, global_min, global_max))
    }

    fn post_process_schedule(&self, mut schedule: HashMap<String, i32>) -> Result<HashMap<String, i32>, String> {
        self.debug_print("🔄", "Post-processing schedule");
        
        // Relax schedule to ensure all constraints are satisfied
        self.debug_print("🧩", "Relaxing schedule to satisfy all constraints");
        self.relax_schedule(&mut schedule)?;
        
        // Final validation to ensure we haven't violated any topological ordering constraints
        self.debug_print("🧮", "Validating topological ordering");
        self.validate_topological_order(&mut schedule)?;
        
        Ok(schedule)
    }

    fn extract_earliest(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("⏱️", "Extracting earliest possible schedule");
        self.extract_with_strategy(|lb, _| lb)
    }
    
    fn extract_latest(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("⏰", "Extracting latest possible schedule");
        self.extract_with_strategy(|_, ub| ub)
    }
    
    fn extract_centered(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("⚖️", "Extracting centered schedule");
        self.extract_with_strategy(|lb, ub| (lb + ub) / 2)
    }

    fn extract_justified_global(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("📏", "Extracting justified global schedule");
        
        // Get all sorted clocks with their bounds and global span
        let (all_vars, global_min, global_max) = self.prepare_global_schedule()?;
    
        let mut schedule = HashMap::new();
    
        // Distribute events evenly across the feasible span
        let total_span = global_max - global_min;
        let count = all_vars.len();
        
        self.debug_print("📊", &format!(
            "Total span: {} minutes with {} clocks", total_span, count
        ));
    
        // Use the first and last bounds but stay within global feasible region
        for (i, (clock_id, lb, ub, _, _)) in all_vars.iter().enumerate() {
            let position: i64;
    
            if i == 0 {
                // First clock at the beginning of span
                position = global_min.max(*lb);
                self.debug_print("🏁", &format!(
                    "First clock {} at position {} (max of global_min {} and lb {})",
                    clock_id, position, global_min, lb
                ));
            } else if i == count - 1 {
                // Last clock at the end of span
                position = global_max.min(*ub);
                self.debug_print("🏁", &format!(
                    "Last clock {} at position {} (min of global_max {} and ub {})",
                    clock_id, position, global_max, ub
                ));
            } else {
                // Intermediate clocks evenly distributed
                position = global_min + (total_span * i as i64) / (count as i64 - 1);
                self.debug_print("📍", &format!(
                    "Clock {} at position {} ({}/{} of the way through span)",
                    clock_id, position, i, count-1
                ));
            }
    
            // Always clamp to this clock's individual bounds
            let clamped = position.clamp(*lb, *ub);
            if clamped != position {
                self.debug_print("📌", &format!(
                    "Clamped {} from {} to {} (bounds: [{}, {}])",
                    clock_id, position, clamped, lb, ub
                ));
            }
            
            self.debug_set_time(clock_id, clamped as i32);
            
            schedule.insert(clock_id.clone(), clamped as i32);
        }
        
        self.post_process_schedule(schedule)
    }
    
    fn extract_max_spread_global(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("↔️", "Extracting maximum spread global schedule");
        
        // Get all sorted clocks with their bounds and global span
        let (all_vars, global_min, global_max) = self.prepare_global_schedule()?;
    
        let total_span = global_max - global_min;
    
        // Calculate ideal separation
        let ideal_gap = if all_vars.len() > 1 {
            total_span / (all_vars.len() as i64 - 1)
        } else {
            0 // If there's only one clock, no gap is needed
        };
        
        self.debug_print("📏", &format!(
            "Ideal gap between events: {} minutes", ideal_gap
        ));
    
        // Create initial schedule with maximum spread
        let mut schedule = HashMap::new();
        for (i, (clock_id, lb, ub, _, _)) in all_vars.iter().enumerate() {
            let ideal_time = global_min + (ideal_gap * i as i64);
            let clamped = ideal_time.clamp(*lb, *ub);
            
            if clamped != ideal_time {
                self.debug_print("📌", &format!(
                    "Clamped {} from ideal {} to {} (bounds: [{}, {}])",
                    clock_id, ideal_time, clamped, lb, ub
                ));
            }
            
            schedule.insert(clock_id.clone(), clamped as i32);
            self.debug_set_time(clock_id, clamped as i32);
        }
        
        self.post_process_schedule(schedule)
    }

    // New method to validate that entity instances are scheduled in topological order
    fn validate_topological_order(
        &self,
        schedule: &mut HashMap<String, i32>,
    ) -> Result<(), String> {
        self.debug_print("🧮", "Validating topological ordering of entity instances");
        
        // Group clocks by entity
        let mut entity_clocks: HashMap<String, Vec<(String, usize, i32)>> = HashMap::new();

        for (clock_id, &time) in schedule.iter() {
            if let Some(info) = self.clocks.get(clock_id) {
                entity_clocks
                    .entry(info.entity_name.clone())
                    .or_insert_with(Vec::new)
                    .push((clock_id.clone(), info.instance, time));
            }
        }

        // Check each entity's clocks are in correct order
        for (entity_name, clocks) in entity_clocks.iter() {
            if clocks.len() <= 1 {
                continue; // Skip entities with only one instance
            }

            self.debug_print("👥", &format!("Checking order for entity: {}", entity_name));

            // Sort by instance number
            let mut ordered_clocks = clocks.clone();
            ordered_clocks.sort_by_key(|&(_, instance, _)| instance);

            // Verify ordering and fix if needed
            for i in 0..ordered_clocks.len() - 1 {
                let (id1, instance1, time1) = &ordered_clocks[i];
                let (id2, instance2, time2) = &ordered_clocks[i + 1];

                self.debug_print("⏱️", &format!(
                    "Checking {} (instance {}, time {}) before {} (instance {}, time {})",
                    id1, instance1, time1, id2, instance2, time2
                ));

                // If later instance is scheduled earlier, adjust it
                if time2 <= time1 {
                    self.debug_error("⚠️", &format!(
                        "Instance ordering violated: {} (time {}) should be after {} (time {})",
                        id2, time2, id1, time1
                    ));
                    
                    // Reschedule the second clock at least 1 minute after the first
                    let new_time = time1 + 1;
                    self.debug_print("🔧", &format!(
                        "Adjusting {} time from {} to {}",
                        id2, time2, new_time
                    ));
                    
                    schedule.insert(id2.clone(), new_time);
                }
            }
        }

        Ok(())
    }

    fn relax_schedule(&self, schedule: &mut HashMap<String, i32>) -> Result<(), String> {
        self.debug_print("🧩", "Relaxing schedule to ensure all constraints are satisfied");
        
        // Iterate until no more changes are needed
        let mut changed = true;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 100; // Safety limit to prevent infinite loops
    
        while changed && iterations < MAX_ITERATIONS {
            changed = false;
            iterations += 1;
            
            self.debug_print("🔄", &format!("Relaxation iteration {}", iterations));
    
            // For each pair of clocks, check all difference constraints from the DBM
            for (i_id, i_info) in self.clocks.iter() {
                let i_var = i_info.variable;
                let i_time = *schedule.get(i_id).unwrap_or(&0);
    
                for (j_id, j_info) in self.clocks.iter() {
                    if i_id == j_id {
                        continue; // Skip same clock
                    }
    
                    let j_var = j_info.variable;
                    let j_time = *schedule.get(j_id).unwrap_or(&0);
    
                    // Check constraint in both directions
                    // First direction: j - i <= bound
                    match self.enforce_constraint(
                        schedule, i_id, i_var, i_time, j_id, j_var, j_time
                    ) {
                        Ok(true) => {
                            changed = true;
                            self.debug_print("✅", &format!(
                                "Adjusted schedule to satisfy constraint between {} and {}",
                                i_id, j_id
                            ));
                        },
                        Err(msg) => {
                            self.debug_error("❌", &format!(
                                "Failed to satisfy constraint: {}", msg
                            ));
                            return Err(msg);
                        },
                        _ => {}
                    }
                    
                    // Second direction: i - j <= bound
                    match self.enforce_constraint(
                        schedule, j_id, j_var, j_time, i_id, i_var, i_time
                    ) {
                        Ok(true) => {
                            changed = true;
                            self.debug_print("✅", &format!(
                                "Adjusted schedule to satisfy constraint between {} and {}",
                                j_id, i_id
                            ));
                        },
                        Err(msg) => {
                            self.debug_error("❌", &format!(
                                "Failed to satisfy constraint: {}", msg
                            ));
                            return Err(msg);
                        },
                        _ => {}
                    }
                }
            }
        }
    
        if iterations >= MAX_ITERATIONS {
            self.debug_error("⚠️", "Failed to stabilize schedule after maximum iterations");
            return Err("Failed to stabilize schedule after maximum iterations".to_string());
        }
        
        self.debug_print("✅", &format!("Schedule relaxed successfully after {} iterations", iterations));
    
        Ok(())
    }
}
