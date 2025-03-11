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

    // Debug methods to match the compiler's style
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
                self.extract_justified_with_constraints()
            },
            ScheduleStrategy::MaximumSpread => {
                self.debug_print("↔️", "Using MaximumSpread strategy - maximizing distance between consecutive events");
                self.extract_max_spread_with_constraints()
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
    fn sort_clocks_topologically(&self) -> Vec<(String, &ClockInfo)> {
        self.debug_print("🔄", "Sorting clocks topologically");

        let mut all_clocks: Vec<(String, &ClockInfo)> = Vec::new();

        // Collect all clocks with their info (as references)
        for (clock_id, info) in self.clocks.iter() {
            all_clocks.push((clock_id.clone(), info));

            if self.debug {
                let bounds = self.get_bounds(info.variable);
                self.debug_bounds(clock_id, &bounds);
            }
        }

        // Sort first by entity name, then by instance number
        all_clocks.sort_by(
            |(_, info_a), (_, info_b)| {
                // First sort by entity name
                let entity_cmp = info_a.entity_name.cmp(&info_b.entity_name);
                if entity_cmp != std::cmp::Ordering::Equal {
                    return entity_cmp;
                }
                // Then by instance number if same entity
                info_a.instance.cmp(&info_b.instance)
            },
        );

        if self.debug {
            self.debug_print("📋", "Sorted clock order:");
            for (i, (id, info)) in all_clocks.iter().enumerate() {
                println!("   {}. {} ({}, instance {})",
                         i+1, id.cyan(), info.entity_name.blue(), info.instance);
            }
        }

        all_clocks
    }


    // Calculate the difference constraint between two clocks
    fn get_difference_constraints(&self, from_var: impl AnyClock + Copy, to_var: impl AnyClock + Copy) -> i64 {
        // If there's a constraint to_var - from_var <= c, then from_var must be at least (-c) after to_var
        // That means: from_var >= to_var + (-c)
        if let Some(bound) = self.zone.get_bound(to_var, from_var).constant() {
            if self.debug {
                self.debug_print("🔗", &format!(
                    "Found constraint: difference must be at least {} minutes", -bound
                ));
            }
            return -bound;
        }

        // If no constraint, return a conservative default
        if self.debug {
            self.debug_print("🔗", "No explicit constraint found, using default (0 minutes)");
        }
        0 // Default: no minimum separation required
    }

    // This implements the "earliest feasible time" approach in a single topological pass
    fn forward_pass(&self, sorted_clocks: &Vec<(String, &ClockInfo)>, schedule: &mut HashMap<String, i32>) {
        self.debug_print("⏩", "Performing forward pass to find earliest feasible times");

        // Start with all clocks at their earliest possible bound
        for (clock_id, info) in sorted_clocks {
            let bounds = self.get_bounds(info.variable);
            schedule.insert(clock_id.clone(), bounds.lb as i32);

            if self.debug {
                self.debug_print("🕒", &format!(
                    "Starting {} at its lower bound: {}",
                    clock_id, bounds.lb
                ));
            }
        }

        // For each clock in topological order:
        for (i, (current_id, current_info)) in sorted_clocks.iter().enumerate() {
            let current_var = current_info.variable;

            // Start with its lower bound
            let mut earliest_time = self.get_bounds(current_var).lb;

            // For all previously assigned clocks, check if they constrain this clock
            for j in 0..i {
                let (prev_id, prev_info) = &sorted_clocks[j];
                let prev_var = prev_info.variable;

                // Get the time for the previous clock
                let prev_time = schedule.get(prev_id).unwrap_or(&0);

                // Check if there's a constraint: current - prev >= min_diff
                let min_diff = self.get_difference_constraints(current_var, prev_var);

                // Update earliest time if needed
                let constraint_earliest = *prev_time as i64 + min_diff;
                if constraint_earliest > earliest_time {
                    if self.debug {
                        self.debug_print("⬆️", &format!(
                            "Clock {} pushes {} to at least {} (was {})",
                            prev_id, current_id, constraint_earliest, earliest_time
                        ));
                    }
                    earliest_time = constraint_earliest;
                }
            }

            // Update the clock's time, ensuring it's within bounds
            let bounds = self.get_bounds(current_var);
            let clamped_time = earliest_time.clamp(bounds.lb, bounds.ub);

            if clamped_time != earliest_time {
                self.debug_print("📌", &format!(
                    "Clamped {} from {} to {} (bounds: [{}, {}])",
                    current_id, earliest_time, clamped_time, bounds.lb, bounds.ub
                ));
            }

            schedule.insert(current_id.clone(), clamped_time as i32);
            self.debug_set_time(current_id, clamped_time as i32);
        }
    }

    // This implements the "latest feasible time" approach in a single reverse topological pass
    fn backward_pass(&self, sorted_clocks: &Vec<(String, &ClockInfo)>, schedule: &mut HashMap<String, i32>) {
        self.debug_print("⏪", "Performing backward pass to find latest feasible times");

        // Start with all clocks at their latest possible bound
        for (clock_id, info) in sorted_clocks {
            let bounds = self.get_bounds(info.variable);
            schedule.insert(clock_id.clone(), bounds.ub as i32);

            if self.debug {
                self.debug_print("🕙", &format!(
                    "Starting {} at its upper bound: {}",
                    clock_id, bounds.ub
                ));
            }
        }

        // For each clock in reverse topological order:
        for i in (0..sorted_clocks.len()).rev() {
            let (current_id, current_info) = &sorted_clocks[i];
            let current_var = current_info.variable;

            // Start with its upper bound
            let mut latest_time = self.get_bounds(current_var).ub;

            // For all clocks that come after this one, check if they constrain this clock
            for j in i+1..sorted_clocks.len() {
                let (next_id, next_info) = &sorted_clocks[j];
                let next_var = next_info.variable;

                // Get the time for the next clock
                let next_time = schedule.get(next_id).unwrap_or(&0);

                // Check if there's a constraint: next - current >= min_diff
                let min_diff = self.get_difference_constraints(next_var, current_var);

                // This implies current <= next - min_diff
                if min_diff > 0 { // If there's an actual minimum separation required
                    let constraint_latest = *next_time as i64 - min_diff;
                    if constraint_latest < latest_time {
                        if self.debug {
                            self.debug_print("⬇️", &format!(
                                "Clock {} pulls {} back to at most {} (was {})",
                                next_id, current_id, constraint_latest, latest_time
                            ));
                        }
                        latest_time = constraint_latest;
                    }
                }
            }

            // Update the clock's time, ensuring it's within bounds
            let bounds = self.get_bounds(current_var);
            let clamped_time = latest_time.clamp(bounds.lb, bounds.ub);

            if clamped_time != latest_time {
                self.debug_print("📌", &format!(
                    "Clamped {} from {} to {} (bounds: [{}, {}])",
                    current_id, latest_time, clamped_time, bounds.lb, bounds.ub
                ));
            }

            schedule.insert(current_id.clone(), clamped_time as i32);
            self.debug_set_time(current_id, clamped_time as i32);
        }
    }


    // Extract earliest schedule using forward pass
    fn extract_earliest(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("⏱️", "Extracting earliest feasible schedule");

        // Sort clocks topologically
        let sorted_clocks = self.sort_clocks_topologically();

        // Use the forward pass to get the earliest feasible schedule
        let mut schedule = HashMap::new();
        self.forward_pass(&sorted_clocks, &mut schedule);

        Ok(schedule)
    }

    // Extract latest schedule using backward pass
    fn extract_latest(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("⏰", "Extracting latest feasible schedule");

        // Sort clocks topologically
        let sorted_clocks = self.sort_clocks_topologically();

        // Use the backward pass to get the latest feasible schedule
        let mut schedule = HashMap::new();
        self.backward_pass(&sorted_clocks, &mut schedule);

        Ok(schedule)
    }

    // Extract centered schedule
    fn extract_centered(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("⚖️", "Extracting centered schedule");

        // Get both earliest and latest schedules
        let sorted_clocks = self.sort_clocks_topologically();

        let mut earliest_schedule = HashMap::new();
        self.forward_pass(&sorted_clocks, &mut earliest_schedule);

        let mut latest_schedule = HashMap::new();
        self.backward_pass(&sorted_clocks, &mut latest_schedule);

        // Create a new schedule with times at the midpoint
        let mut centered_schedule = HashMap::new();

        for (clock_id, _) in &sorted_clocks {
            let earliest = *earliest_schedule.get(clock_id).unwrap_or(&0) as f64;
            let latest = *latest_schedule.get(clock_id).unwrap_or(&1440) as f64;

            // Use the midpoint
            let centered_time = ((earliest + latest) / 2.0) as i32;

            self.debug_print("↔️", &format!(
                "Centering {} between {} and {} at {}",
                clock_id, earliest as i32, latest as i32, centered_time
            ));

            centered_schedule.insert(clock_id.clone(), centered_time);
            self.debug_set_time(clock_id, centered_time);
        }

        self.debug_print("🔄", "Verifying and fixing any constraint violations");
        self.fix_constraint_violations(&sorted_clocks, &mut centered_schedule);

        Ok(centered_schedule)
    }

    // Justified schedule that respects constraints
    fn extract_justified_with_constraints(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("📏", "Extracting justified schedule that respects constraints");

        // Sort clocks topologically
        let sorted_clocks = self.sort_clocks_topologically();
        if sorted_clocks.is_empty() {
            return Err("No clocks found to schedule".to_string());
        }

        // Get earliest feasible times with a forward pass
        let mut earliest_schedule = HashMap::new();
        self.forward_pass(&sorted_clocks, &mut earliest_schedule);

        // Get latest feasible times with a backward pass
        let mut latest_schedule = HashMap::new();
        self.backward_pass(&sorted_clocks, &mut latest_schedule);

        // Find the global earliest and latest times
        let mut global_earliest = i32::MAX;
        let mut global_latest = i32::MIN;

        for (clock_id, _) in &sorted_clocks {
            let earliest = *earliest_schedule.get(clock_id).unwrap_or(&0);
            let latest = *latest_schedule.get(clock_id).unwrap_or(&1440);

            if earliest < global_earliest {
                global_earliest = earliest;
            }
            if latest > global_latest {
                global_latest = latest;
            }
        }

        self.debug_print("🌐", &format!(
            "Global feasible range: {} - {} (span of {} minutes)",
            global_earliest, global_latest, global_latest - global_earliest
        ));

        let mut justified_schedule = HashMap::new();

        // Create a justified schedule within the valid range for each clock
        let n_clocks = sorted_clocks.len() as f64;

        for (i, (clock_id, _)) in sorted_clocks.iter().enumerate() {
            let earliest = *earliest_schedule.get(clock_id).unwrap_or(&0) as f64;
            let latest = *latest_schedule.get(clock_id).unwrap_or(&1440) as f64;

            let justified_time: i32;

            // If this is the first or last clock, use the earliest or latest time
            if i == 0 {
                justified_time = earliest as i32;
                self.debug_print("🏁", &format!(
                    "First clock {} at earliest possible time {}",
                    clock_id, justified_time
                ));
            } else if i == sorted_clocks.len() - 1 {
                justified_time = latest as i32;
                self.debug_print("🏁", &format!(
                    "Last clock {} at latest possible time {}",
                    clock_id, justified_time
                ));
            } else {
                // Otherwise, distribute proportionally within the feasible range
                let global_span = global_latest as f64 - global_earliest as f64;
                let fraction = i as f64 / (n_clocks - 1.0);
                let target_time = global_earliest as f64 + fraction * global_span;

                // Clamp to this clock's feasible range
                justified_time = if target_time < earliest {
                    self.debug_print("📍", &format!(
                        "Clock {} target {} pushed forward to {} (its earliest feasible time)",
                        clock_id, target_time as i32, earliest as i32
                    ));
                    earliest as i32
                } else if target_time > latest {
                    self.debug_print("📍", &format!(
                        "Clock {} target {} pulled back to {} (its latest feasible time)",
                        clock_id, target_time as i32, latest as i32
                    ));
                    latest as i32
                } else {
                    self.debug_print("📍", &format!(
                        "Clock {} at {}/{} of span: {}",
                        clock_id, i, sorted_clocks.len()-1, target_time as i32
                    ));
                    target_time as i32
                };
            }

            justified_schedule.insert(clock_id.clone(), justified_time);
            self.debug_set_time(clock_id, justified_time);
        }

        // REMOVED: Final forward pass to ensure all constraints are satisfied
        // Instead, we'll do a more careful check and only adjust when needed
        self.debug_print("🔄", "Verifying and fixing any constraint violations");
        self.fix_constraint_violations(&sorted_clocks, &mut justified_schedule);

        Ok(justified_schedule)
    }

    // Maximum Spread schedule that respects constraints
    fn extract_max_spread_with_constraints(&self) -> Result<HashMap<String, i32>, String> {
        self.debug_print("↔️", "Extracting maximum spread schedule that respects constraints");

        // Sort clocks topologically
        let sorted_clocks = self.sort_clocks_topologically();
        if sorted_clocks.is_empty() {
            return Err("No clocks found to schedule".to_string());
        }

        // Get earliest and latest feasible times
        let mut earliest_schedule = HashMap::new();
        self.forward_pass(&sorted_clocks, &mut earliest_schedule);

        let mut latest_schedule = HashMap::new();
        self.backward_pass(&sorted_clocks, &mut latest_schedule);

        // Get earliest time for the first clock
        let mut global_earliest = i32::MAX;

        for (clock_id, _) in &sorted_clocks {
            let earliest = *earliest_schedule.get(clock_id).unwrap_or(&0);

            if earliest < global_earliest {
                global_earliest = earliest;
            }
        }

        // Find the latest time for the last clock
        let (last_id, _) = &sorted_clocks[sorted_clocks.len() - 1];
        let global_latest = *latest_schedule.get(last_id).unwrap_or(&1440);

        self.debug_print("🌐", &format!(
            "Global schedule range: {} to {} (span of {} minutes)",
            global_earliest, global_latest, global_latest - global_earliest
        ));

        // Calculate ideally evenly distributed schedule
        let span = (global_latest - global_earliest) as f64;
        let n_clocks = sorted_clocks.len() as f64;

        // Create a new schedule with maximum spread
        let mut spread_schedule = HashMap::new();

        for (i, (clock_id, _)) in sorted_clocks.iter().enumerate() {
            let earliest = *earliest_schedule.get(clock_id).unwrap_or(&0);
            let latest = *latest_schedule.get(clock_id).unwrap_or(&1440);

            // Calculate ideal position
            let fraction = if n_clocks > 1.0 { i as f64 / (n_clocks - 1.0) } else { 0.0 };
            let ideal_time = (global_earliest as f64 + fraction * span) as i32;

            // Clamp to this clock's feasible range
            let spread_time = if ideal_time < earliest {
                self.debug_print("📍", &format!(
                    "Clock {} ideal {} pushed forward to {} (its earliest feasible time)",
                    clock_id, ideal_time, earliest
                ));
                earliest
            } else if ideal_time > latest {
                self.debug_print("📍", &format!(
                    "Clock {} ideal {} pulled back to {} (its latest feasible time)",
                    clock_id, ideal_time, latest
                ));
                latest
            } else {
                self.debug_print("📍", &format!(
                    "Clock {} at {}/{} of span: {}",
                    clock_id, i, sorted_clocks.len()-1, ideal_time
                ));
                ideal_time
            };

            spread_schedule.insert(clock_id.clone(), spread_time);
            self.debug_set_time(clock_id, spread_time);
        }

        // REMOVED: Final forward pass to ensure all constraints are satisfied
        // Instead, we'll do a more careful check and only adjust when needed
        self.debug_print("🔄", "Verifying and fixing any constraint violations");
        self.fix_constraint_violations(&sorted_clocks, &mut spread_schedule);

        Ok(spread_schedule)
    }


    // Fix constraints without resetting the entire schedule
    fn fix_constraint_violations(&self, sorted_clocks: &Vec<(String, &ClockInfo)>, schedule: &mut HashMap<String, i32>) {
        let mut changed = true;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 10;

        while changed && iterations < MAX_ITERATIONS {
            changed = false;
            iterations += 1;

            self.debug_print("🔄", &format!("Constraint verification pass {}", iterations));

            // Check all pairs of clocks for constraint violations
            for (i, (id_i, info_i)) in sorted_clocks.iter().enumerate() {
                let time_i = *schedule.get(id_i).unwrap_or(&0);

                for (j, (id_j, info_j)) in sorted_clocks.iter().enumerate() {
                    if i == j {
                        continue;
                    }

                    let time_j = *schedule.get(id_j).unwrap_or(&0);

                    // Check if there is a constraint from i to j
                    let min_diff = self.get_difference_constraints(info_j.variable, info_i.variable);

                    if min_diff > 0 {
                        // There is a constraint: j must be at least min_diff after i
                        if time_j < time_i + min_diff as i32 {
                            let constraint_violated = format!(
                                "{} must be at least {} minutes after {}, but it's only {} minutes",
                                id_j, min_diff, id_i, time_j - time_i
                            );
                            self.debug_error("⚠️", &constraint_violated);

                            // Fix by adjusting j forward (preferred if possible)
                            let new_time_j = time_i + min_diff as i32;
                            let j_bounds = self.get_bounds(info_j.variable);

                            if new_time_j <= j_bounds.ub as i32 {
                                self.debug_print("🔧", &format!(
                                    "Fixing by moving {} forward from {} to {}",
                                    id_j, time_j, new_time_j
                                ));

                                schedule.insert(id_j.clone(), new_time_j);
                                changed = true;
                            } else {
                                // If can't move j forward, try moving i backward
                                let new_time_i = time_j - min_diff as i32;
                                let i_bounds = self.get_bounds(info_i.variable);

                                if new_time_i >= i_bounds.lb as i32 {
                                    self.debug_print("🔧", &format!(
                                        "Fixing by moving {} backward from {} to {}",
                                        id_i, time_i, new_time_i
                                    ));

                                    schedule.insert(id_i.clone(), new_time_i);
                                    changed = true;
                                } else {
                                    // Can't fix this constraint within bounds
                                    self.debug_error("❌", &format!(
                                        "Cannot fix constraint between {} and {}: bounds too restrictive",
                                        id_i, id_j
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        if iterations >= MAX_ITERATIONS {
            self.debug_error("⚠️", "Reached maximum iterations for constraint fixing. Schedule may not be fully optimal.");
        } else {
            self.debug_print("✅", &format!("Constraint verification complete after {} passes", iterations));
        }

        // Manual check for instance ordering instead of calling validate_topological_order
        self.debug_print("🧮", "Verifying instance ordering");

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
    }
}
