use clock_zones::{Bound, Dbm, Variable, Zone};
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

pub struct ScheduleExtractor<'a> {
    pub zone: &'a Dbm<i64>,
    pub clocks: &'a HashMap<String, ClockInfo>,
}

impl<'a> ScheduleExtractor<'a> {
    pub fn new(zone: &'a Dbm<i64>, clocks: &'a HashMap<String, ClockInfo>) -> Self {
        Self { zone, clocks }
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
        match strategy {
            ScheduleStrategy::Earliest => self.extract_earliest(),
            ScheduleStrategy::Latest => self.extract_latest(),
            ScheduleStrategy::Centered => self.extract_centered(),
            ScheduleStrategy::Justified => self.extract_justified_global(),
            ScheduleStrategy::MaximumSpread => self.extract_max_spread_global(),
        }
    }

    fn extract_earliest(&self) -> Result<HashMap<String, i32>, String> {
        let mut schedule = HashMap::new();
        for (clock_id, info) in self.clocks.iter() {
            let lb = self.zone.get_lower_bound(info.variable).unwrap_or(0);
            schedule.insert(clock_id.clone(), lb as i32);
        }
        Ok(schedule)
    }

    fn extract_latest(&self) -> Result<HashMap<String, i32>, String> {
        let mut schedule = HashMap::new();
        for (clock_id, info) in self.clocks.iter() {
            let ub = self.zone.get_upper_bound(info.variable).unwrap_or(1440);
            schedule.insert(clock_id.clone(), ub as i32);
        }
        Ok(schedule)
    }

    fn extract_centered(&self) -> Result<HashMap<String, i32>, String> {
        let mut schedule = HashMap::new();
        for (clock_id, info) in self.clocks.iter() {
            let lb = self.zone.get_lower_bound(info.variable).unwrap_or(0);
            let ub = self.zone.get_upper_bound(info.variable).unwrap_or(1440);
            let mid = (lb + ub) / 2;
            schedule.insert(clock_id.clone(), mid as i32);
        }
        Ok(schedule)
    }

    fn extract_justified_global(&self) -> Result<HashMap<String, i32>, String> {
        // 1) Collect all clocks with their bounds
        let mut all_vars: Vec<(String, i64, i64)> = Vec::new();
        for (clock_id, info) in &*self.clocks {
            let lb = self.zone.get_lower_bound(info.variable).unwrap_or(0);
            let ub = self.zone.get_upper_bound(info.variable).unwrap_or(1440);
            all_vars.push((clock_id.clone(), lb, ub));
        }

        // 2) Sort them by lb ascending
        all_vars.sort_by_key(|(_, lb, _)| *lb);

        // Edge case: only one clock
        if all_vars.len() <= 1 {
            return self.extract_centered();
        }

        // 3) Pin first to its lb, last to its ub
        let (first_id, first_lb, _) = &all_vars[0];
        let (last_id, _, last_ub) = &all_vars[all_vars.len() - 1];
        let start = *first_lb;
        let end = *last_ub;

        let mut schedule = HashMap::new();
        schedule.insert(first_id.clone(), start as i32);
        schedule.insert(last_id.clone(), end as i32);

        // 4) Distribute the middle clocks
        let total_span = end - start;
        for i in 1..all_vars.len() - 1 {
            let (clk_id, lb, ub) = &all_vars[i];

            // Skip if we already set it (could be the same as first or last)
            if clk_id == first_id || clk_id == last_id {
                continue;
            }

            let ideal = start + (total_span * i as i64) / ((all_vars.len() - 1) as i64);
            let clamped = ideal.clamp(*lb, *ub);
            schedule.insert(clk_id.clone(), clamped as i32);
        }

        // 5) Relax schedule to ensure all constraints are satisfied
        self.relax_schedule(&mut schedule)?;

        Ok(schedule)
    }

    fn extract_max_spread_global(&self) -> Result<HashMap<String, i32>, String> {
        // For max spread, we use a similar approach to justified, but we start by
        // calculating the ideal spacing between events

        // 1) Collect all clocks with their bounds
        let mut all_vars: Vec<(String, i64, i64)> = Vec::new();
        for (clock_id, info) in &*self.clocks {
            let lb = self.zone.get_lower_bound(info.variable).unwrap_or(0);
            let ub = self.zone.get_upper_bound(info.variable).unwrap_or(1440);
            all_vars.push((clock_id.clone(), lb, ub));
        }

        // Edge case: only one clock
        if all_vars.len() <= 1 {
            return self.extract_centered();
        }

        // Find overall bounds of the entire schedule
        let global_min = all_vars.iter().map(|(_, lb, _)| *lb).min().unwrap_or(0);
        let global_max = all_vars.iter().map(|(_, _, ub)| *ub).max().unwrap_or(1440);
        let total_span = global_max - global_min;

        // Calculate ideal separation
        let ideal_gap = total_span / (all_vars.len() as i64 - 1);

        // Sort by midpoint to get an ordering for even distribution
        all_vars.sort_by_key(|(_, lb, ub)| (lb + ub) / 2);

        // Create initial schedule with maximum spread
        let mut schedule = HashMap::new();
        for (i, (clock_id, lb, ub)) in all_vars.iter().enumerate() {
            let ideal_time = global_min + (ideal_gap * i as i64);
            let clamped = ideal_time.clamp(*lb, *ub);
            schedule.insert(clock_id.clone(), clamped as i32);
        }

        // Relax schedule to ensure all constraints are satisfied
        self.relax_schedule(&mut schedule)?;

        Ok(schedule)
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

    fn find_clock_id(&self, var: Variable) -> Result<String, String> {
        for (id, info) in self.clocks {
            if info.variable == var {
                return Ok(id.clone());
            }
        }
        Err(format!("No clock ID found for variable {:?}", var))
    }
}
