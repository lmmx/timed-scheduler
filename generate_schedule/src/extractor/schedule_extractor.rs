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

    fn is_within_bounds(&self, variable: impl AnyClock, time: i32) -> bool {
        let bounds = self.get_bounds(variable);
        time >= bounds.lb as i32 && time <= bounds.ub as i32
    }
    
    fn clamp_to_bounds(&self, variable: impl AnyClock, time: i32) -> i32 {
        let bounds = self.get_bounds(variable);
        time.clamp(bounds.lb as i32, bounds.ub as i32)
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
                // Try adjusting the second clock first
                let new_second_time = first_time + bound as i32;
                let second_bounds = self.get_bounds(second_var);
                let second_ub = second_bounds.ub as i32;
    
                // Make sure the new time is within second clock's upper bound
                if new_second_time <= second_ub {
                    schedule.insert(second_id.to_string(), new_second_time);
                    return Ok(true);
                }
                
                // If we can't move second clock down enough, try moving first clock up
                let new_first_time = second_time - bound as i32;
                let first_bounds = self.get_bounds(first_var);
                let first_lb = first_bounds.lb as i32;
    
                if new_first_time >= first_lb {
                    schedule.insert(first_id.to_string(), new_first_time);
                    return Ok(true);
                }
                
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
        let mut schedule = HashMap::new();
        for (clock_id, info) in self.clocks.iter() {
            let bounds = self.get_bounds(info.variable);
            let time = time_selector(bounds.lb, bounds.ub);
            schedule.insert(clock_id.clone(), time as i32);
        }
        Ok(schedule)
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
                if !self.is_within_bounds(info.variable, *time) {
                    // Clamp to valid range
                    *time = self.clamp_to_bounds(info.variable, *time);
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

    fn prepare_global_schedule(&self) -> Result<(Vec<(String, i64, i64, usize, String)>, i64, i64), String> {
        // Collect all clocks with their bounds
        let all_vars = self.sort_clocks_topologically();
        
        if all_vars.is_empty() {
            return Err("No clocks found to schedule".to_string());
        }
    
        // Find the feasible span for the entire schedule
        let global_min = all_vars.iter().map(|(_, lb, _, _, _)| *lb).max().unwrap_or(0);
        let global_max = all_vars.iter().map(|(_, _, ub, _, _)| *ub).min().unwrap_or(1440);
    
        // Safety check: ensure we have a valid span
        if global_min >= global_max {
            return Err(format!(
                "No valid global span available: min={}, max={}", 
                global_min, global_max
            ));
        }
    
        Ok((all_vars, global_min, global_max))
    }

    fn post_process_schedule(&self, mut schedule: HashMap<String, i32>) -> Result<HashMap<String, i32>, String> {
        // Relax schedule to ensure all constraints are satisfied
        self.relax_schedule(&mut schedule)?;
        // Final validation to ensure we haven't violated any topological ordering constraints
        self.validate_topological_order(&mut schedule)?;
        Ok(schedule)
    }

    fn extract_earliest(&self) -> Result<HashMap<String, i32>, String> {
        self.extract_with_strategy(|lb, _| lb)
    }
    
    fn extract_latest(&self) -> Result<HashMap<String, i32>, String> {
        self.extract_with_strategy(|_, ub| ub)
    }
    
    fn extract_centered(&self) -> Result<HashMap<String, i32>, String> {
        self.extract_with_strategy(|lb, ub| (lb + ub) / 2)
    }

    fn extract_justified_global(&self) -> Result<HashMap<String, i32>, String> {
        // Get all sorted clocks with their bounds and global span
        let (all_vars, global_min, global_max) = self.prepare_global_schedule()?;
    
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
        
        self.post_process_schedule(schedule)
    }
    
    fn extract_max_spread_global(&self) -> Result<HashMap<String, i32>, String> {
        // Get all sorted clocks with their bounds and global span
        let (all_vars, global_min, global_max) = self.prepare_global_schedule()?;
    
        let total_span = global_max - global_min;
    
        // Calculate ideal separation
        let ideal_gap = if all_vars.len() > 1 {
            total_span / (all_vars.len() as i64 - 1)
        } else {
            0 // If there's only one clock, no gap is needed
        };
    
        // Create initial schedule with maximum spread
        let mut schedule = HashMap::new();
        for (i, (clock_id, lb, ub, _, _)) in all_vars.iter().enumerate() {
            let ideal_time = global_min + (ideal_gap * i as i64);
            let clamped = ideal_time.clamp(*lb, *ub);
            schedule.insert(clock_id.clone(), clamped as i32);
        }
        
        self.post_process_schedule(schedule)
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
    
                    // Check constraint in both directions
                    // First direction: j - i <= bound
                    match self.enforce_constraint(
                        schedule, i_id, i_var, i_time, j_id, j_var, j_time
                    ) {
                        Ok(true) => changed = true,
                        Err(msg) => return Err(msg),
                        _ => {}
                    }
                    
                    // Second direction: i - j <= bound
                    match self.enforce_constraint(
                        schedule, j_id, j_var, j_time, i_id, i_var, i_time
                    ) {
                        Ok(true) => changed = true,
                        Err(msg) => return Err(msg),
                        _ => {}
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
