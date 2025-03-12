use std::collections::HashMap;
use std::env;
use clock_zones::{AnyClock, Bound, Dbm, Zone};
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
pub struct Bounds {
    pub lb: i64,
    pub ub: i64,
}

pub struct ScheduleExtractor<'a> {
    pub zone: &'a Dbm<i64>,
    pub clocks: &'a HashMap<String, ClockInfo>,
    pub debug: bool,
}

impl<'a> ScheduleExtractor<'a> {
    pub fn new(zone: &'a Dbm<i64>, clocks: &'a HashMap<String, ClockInfo>) -> Self {
        // Check if debug flag is set - same approach as compiler
        let debug = env::var("RUST_DEBUG").is_ok() || env::args().any(|arg| arg == "--debug");

        Self { zone, clocks, debug }
    }

    pub fn get_bounds(&self, variable: impl AnyClock) -> Bounds {
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
                crate::extractor::strategies::extract_earliest(self)
            },
            ScheduleStrategy::Latest => {
                self.debug_print("⏰", "Using Latest strategy - placing all events at their latest possible times");
                crate::extractor::strategies::extract_latest(self)
            },
            ScheduleStrategy::Centered => {
                self.debug_print("⚖️", "Using Centered strategy - placing all events at the middle of their feasible ranges");
                crate::extractor::strategies::extract_centered(self)
            },
            ScheduleStrategy::Justified => {
                self.debug_print("📏", "Using Justified strategy - distributing events to span the entire feasible range");
                crate::extractor::strategies::extract_justified_with_constraints(self)
            },
            ScheduleStrategy::MaximumSpread => {
                self.debug_print("↔️", "Using MaximumSpread strategy - maximizing distance between consecutive events");
                crate::extractor::strategies::extract_max_spread_with_constraints(self)
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

    // Calculate the difference constraint between two clocks
    pub fn get_difference_constraints(&self, from_var: impl AnyClock + Copy, to_var: impl AnyClock + Copy) -> i64 {
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
}
