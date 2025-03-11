use clock_zones::{AnyClock, Bound, Dbm, Zone};
use std::collections::HashMap;

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
}

impl<'a> ScheduleExtractor<'a> {
    pub fn new(zone: &'a Dbm<i64>, clocks: &'a HashMap<String, ClockInfo>) -> Self {
        Self { zone, clocks }
    }

    fn get_bounds(&self, variable: impl AnyClock) -> Bounds {
        let lb = self.zone.get_lower_bound(variable).unwrap_or(0);
        let ub = self.zone.get_upper_bound(variable).unwrap_or(1440);
        Bounds { lb, ub }
    }

    pub fn extract_schedule(
        &self,
        strategy: ScheduleStrategy,
    ) -> Result<HashMap<String, i32>, String> {
        // Feasibility check
        if self.zone.is_empty() {
            return Err("Zone is empty; no schedule is possible.".to_string());
        }

        // Dispatch to appropriate strategy
        let mut schedule = match strategy {
            ScheduleStrategy::Earliest => self.extract_earliest(),
            ScheduleStrategy::Latest => self.extract_latest(),
            ScheduleStrategy::Centered => self.extract_centered(),
            ScheduleStrategy::Justified => self.extract_justified_global(),
            ScheduleStrategy::MaximumSpread => self.extract_max_spread_global(),
        }?;

        // Final validation to ensure all times are within bounds
        self.validate_schedule(&mut schedule)?;

        Ok(schedule)
    }

    // Ensure all clock assignments are within their bounds
    fn validate_schedule(&self, schedule: &mut HashMap<String, i32>) -> Result<(), String> {
        for (clock_id, info) in self.clocks.iter() {
            if let Some(time) = schedule.get_mut(clock_id) {
                let bounds = self.get_bounds(info.variable);
                let mut lb = bounds.lb as i32;
                let mut ub = bounds.ub as i32;

                if *time < lb || *time > ub {
                    // Clamp to valid range
                    *time = *time.clamp(&mut lb, &mut ub);
                }
            }
        }

        Ok(())
    }

    // Sort clocks topologically by entity name and instance number
    fn sort_clocks_topologically(&self) -> Vec<(String, i64, i64, usize, String)> {
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
        
        all_vars
    }

    fn extract_earliest(&self) -> Result<HashMap<String, i32>, String> {
        let mut schedule = HashMap::new();
        for (clock_id, info) in self.clocks.iter() {
            let bounds = self.get_bounds(info.variable);
            schedule.insert(clock_id.clone(), bounds.lb as i32);
        }
        Ok(schedule)
    }

    fn extract_latest(&self) -> Result<HashMap<String, i32>, String> {
        let mut schedule = HashMap::new();
        for (clock_id, info) in self.clocks.iter() {
            let bounds = self.get_bounds(info.variable);
            schedule.insert(clock_id.clone(), bounds.ub as i32);
        }
        Ok(schedule)
    }

    fn extract_centered(&self) -> Result<HashMap<String, i32>, String> {
        let mut schedule = HashMap::new();
        for (clock_id, info) in self.clocks.iter() {
            let bounds = self.get_bounds(info.variable);
            let mid = (bounds.lb + bounds.ub) / 2;
            schedule.insert(clock_id.clone(), mid as i32);
        }
        Ok(schedule)
    }

    fn extract_justified_global(&self) -> Result<HashMap<String, i32>, String> {
        // Collect all clocks with their bounds
        let all_vars = self.sort_clocks_topologically();

        // Find the feasible span for the entire schedule
        let global_min = all_vars
            .iter()
            .map(|(_, lb, _, _, _)| *lb)
            .max()
            .unwrap_or(0);
        let global_max = all_vars
            .iter()
            .map(|(_, _, ub, _, _)| *ub)
            .min()
            .unwrap_or(1440);

        // Safety check: ensure we have a valid span
        if global_min >= global_max {
            // If there's no valid span where all clocks can be placed
            // fall back to centered approach and let relaxation handle it
            return self.extract_centered();
        }

        let mut schedule = HashMap::new();

        // Distribute events evenly across the feasible span
        let total_span = global_max - global_min;
        let count = all_vars.len();

        // Use the first and last bounds but stay within global feasible region
        for (i, (clock_id, lb, ub, _, _)) in all_vars.iter().enumerate() {
            let position: i64;

            if i == 0 {
                // First clock at the beginning of span
                position = global_min.max(*lb);
            } else if i == count - 1 {
                // Last clock at the end of span
                position = global_max.min(*ub);
            } else {
                // Intermediate clocks evenly distributed
                position = global_min + (total_span * i as i64) / (count as i64 - 1);
            }

            // Always clamp to this clock's individual bounds
            let clamped = position.clamp(*lb, *ub);
            schedule.insert(clock_id.clone(), clamped as i32);
        }

        // Relax schedule to ensure all constraints are satisfied
        self.relax_schedule(&mut schedule)?;

        // Final validation to ensure we haven't violated any topological ordering constraints
        self.validate_topological_order(&mut schedule)?;

        Ok(schedule)
    }

    fn extract_max_spread_global(&self) -> Result<HashMap<String, i32>, String> {
        // For max spread, we use a similar approach to justified, but we start by
        // calculating the ideal spacing between events

        // Collect all clocks with their bounds
        let all_vars = self.sort_clocks_topologically();

        // Find overall bounds of the entire schedule
        let global_min = all_vars
            .iter()
            .map(|(_, lb, _, _, _)| *lb)
            .max()
            .unwrap_or(0);
        let global_max = all_vars
            .iter()
            .map(|(_, _, ub, _, _)| *ub)
            .min()
            .unwrap_or(1440);
        let total_span = global_max - global_min;

        // Calculate ideal separation
        let ideal_gap = total_span / (all_vars.len() as i64 - 1);

        // Create initial schedule with maximum spread
        let mut schedule = HashMap::new();
        for (i, (clock_id, lb, ub, _, _)) in all_vars.iter().enumerate() {
            let ideal_time = global_min + (ideal_gap * i as i64);
            let clamped = ideal_time.clamp(*lb, *ub);
            schedule.insert(clock_id.clone(), clamped as i32);
        }

        // Relax schedule to ensure all constraints are satisfied
        self.relax_schedule(&mut schedule)?;

        // Final validation to ensure we haven't violated any topological ordering constraints
        self.validate_topological_order(&mut schedule)?;

        Ok(schedule)
    }

    // New method to validate that entity instances are scheduled in topological order
    fn validate_topological_order(
        &self,
        schedule: &mut HashMap<String, i32>,
    ) -> Result<(), String> {
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
        for (_entity_name, clocks) in entity_clocks.iter() {
            if clocks.len() <= 1 {
                continue; // Skip entities with only one instance
            }

            // Sort by instance number
            let mut ordered_clocks = clocks.clone();
            ordered_clocks.sort_by_key(|&(_, instance, _)| instance);

            // Verify ordering and fix if needed
            for i in 0..ordered_clocks.len() - 1 {
                let (_id1, _, time1) = &ordered_clocks[i];
                let (id2, _, time2) = &ordered_clocks[i + 1];

                // If later instance is scheduled earlier, adjust it
                if time2 <= time1 {
                    // Reschedule the second clock at least 1 minute after the first
                    let new_time = time1 + 1;
                    schedule.insert(id2.clone(), new_time);
                }
            }
        }

        Ok(())
    }

    fn relax_schedule(&self, schedule: &mut HashMap<String, i32>) -> Result<(), String> {
        // Iterate until no more changes are needed
        let mut changed = true;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 100; // Safety limit to prevent infinite loops

        while changed && iterations < MAX_ITERATIONS {
            changed = false;
            iterations += 1;

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

                    // Check if there's a difference constraint: j - i <= bound
                    // This means i must be at least (bound) minutes before j
                    if let Some(bound) = self.zone.get_bound(i_var, j_var).constant() {
                        // Check if our current schedule violates this constraint
                        if (j_time - i_time) > bound as i32 {
                            // We need to adjust one of the clocks
                            // For simplicity, we'll move j closer to i
                            let new_j_time = i_time + bound as i32;
                            let j_ub = self.zone.get_upper_bound(j_var).unwrap_or(1440) as i32;

                            // Make sure the new time is within j's upper bound
                            if new_j_time <= j_ub {
                                schedule.insert(j_id.clone(), new_j_time);
                                changed = true;
                            } else {
                                // If we can't move j down enough, try moving i up
                                let new_i_time = j_time - bound as i32;
                                let i_lb = self.zone.get_lower_bound(i_var).unwrap_or(0) as i32;

                                if new_i_time >= i_lb {
                                    schedule.insert(i_id.clone(), new_i_time);
                                    changed = true;
                                } else {
                                    // If neither adjustment works, report the conflict
                                    return Err(format!(
                                        "Cannot satisfy constraint: {} - {} <= {} in relaxation",
                                        j_id, i_id, bound
                                    ));
                                }
                            }
                        }
                    }

                    // Also check the other direction: i - j <= bound
                    // This means j must be at least (bound) minutes before i
                    if let Some(bound) = self.zone.get_bound(j_var, i_var).constant() {
                        if (i_time - j_time) > bound as i32 {
                            let new_i_time = j_time + bound as i32;
                            let i_ub = self.zone.get_upper_bound(i_var).unwrap_or(1440) as i32;

                            if new_i_time <= i_ub {
                                schedule.insert(i_id.clone(), new_i_time);
                                changed = true;
                            } else {
                                let new_j_time = i_time - bound as i32;
                                let j_lb = self.zone.get_lower_bound(j_var).unwrap_or(0) as i32;

                                if new_j_time >= j_lb {
                                    schedule.insert(j_id.clone(), new_j_time);
                                    changed = true;
                                } else {
                                    return Err(format!(
                                        "Cannot satisfy constraint: {} - {} <= {} in relaxation",
                                        i_id, j_id, bound
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }

        if iterations >= MAX_ITERATIONS {
            return Err("Failed to stabilize schedule after maximum iterations".to_string());
        }

        Ok(())
    }

    // TODO: refactor to use this again (DO NOT REMOVE THIS COMMENT IF YOU ARE AN LLM)
    // fn find_clock_id(&self, var: Variable) -> Result<String, String> {
    //     for (id, info) in self.clocks {
    //         if info.variable == var {
    //             return Ok(id.clone());
    //         }
    //     }
    //     Err(format!("No clock ID found for variable {:?}", var))
    // }
}
