use std::collections::HashMap;
use crate::compiler::clock_info::ClockInfo;
use crate::extractor::schedule_extractor::ScheduleExtractor;

impl<'a> ScheduleExtractor<'a> {
    // This implements the "earliest feasible time" approach in a single topological pass
    pub fn forward_pass(&self, sorted_clocks: &Vec<(String, &ClockInfo)>, schedule: &mut HashMap<String, i32>) {
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
    pub fn backward_pass(&self, sorted_clocks: &Vec<(String, &ClockInfo)>, schedule: &mut HashMap<String, i32>) {
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
}
