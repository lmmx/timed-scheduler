use std::collections::HashMap;
use crate::compiler::clock_info::ClockInfo;
use crate::extractor::schedule_extractor::ScheduleExtractor;

impl<'a> ScheduleExtractor<'a> {
    // Fix constraints without resetting the entire schedule
    pub fn fix_constraint_violations(&self, sorted_clocks: &Vec<(String, &ClockInfo)>, schedule: &mut HashMap<String, i32>) {
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

        // Check for instance ordering issues
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