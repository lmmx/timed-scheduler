use std::collections::HashMap;
use crate::extractor::schedule_extractor::ScheduleExtractor;

// Extract earliest schedule using forward pass
pub fn extract_earliest(extractor: &ScheduleExtractor) -> Result<HashMap<String, i32>, String> {
    extractor.debug_print("⏱️", "Extracting earliest feasible schedule");

    // Sort clocks topologically
    let sorted_clocks = extractor.sort_clocks_topologically();

    // Use the forward pass to get the earliest feasible schedule
    let mut schedule = HashMap::new();
    extractor.forward_pass(&sorted_clocks, &mut schedule);

    Ok(schedule)
}

// Extract latest schedule using backward pass
pub fn extract_latest(extractor: &ScheduleExtractor) -> Result<HashMap<String, i32>, String> {
    extractor.debug_print("⏰", "Extracting latest feasible schedule");

    // Sort clocks topologically
    let sorted_clocks = extractor.sort_clocks_topologically();

    // Use the backward pass to get the latest feasible schedule
    let mut schedule = HashMap::new();
    extractor.backward_pass(&sorted_clocks, &mut schedule);

    Ok(schedule)
}

// Extract centered schedule
pub fn extract_centered(extractor: &ScheduleExtractor) -> Result<HashMap<String, i32>, String> {
    extractor.debug_print("⚖️", "Extracting centered schedule");

    // Get both earliest and latest schedules
    let sorted_clocks = extractor.sort_clocks_topologically();

    let mut earliest_schedule = HashMap::new();
    extractor.forward_pass(&sorted_clocks, &mut earliest_schedule);

    let mut latest_schedule = HashMap::new();
    extractor.backward_pass(&sorted_clocks, &mut latest_schedule);

    // Create a new schedule with times at the midpoint
    let mut centered_schedule = HashMap::new();

    for (clock_id, _) in &sorted_clocks {
        let earliest = *earliest_schedule.get(clock_id).unwrap_or(&0) as f64;
        let latest = *latest_schedule.get(clock_id).unwrap_or(&1440) as f64;

        // Use the midpoint
        let centered_time = ((earliest + latest) / 2.0) as i32;

        extractor.debug_print("↔️", &format!(
            "Centering {} between {} and {} at {}",
            clock_id, earliest as i32, latest as i32, centered_time
        ));

        centered_schedule.insert(clock_id.clone(), centered_time);
        extractor.debug_set_time(clock_id, centered_time);
    }

    extractor.debug_print("🔄", "Verifying and fixing any constraint violations");
    extractor.fix_constraint_violations(&sorted_clocks, &mut centered_schedule);

    Ok(centered_schedule)
}

// Justified schedule that respects constraints
pub fn extract_justified_with_constraints(extractor: &ScheduleExtractor) -> Result<HashMap<String, i32>, String> {
    extractor.debug_print("📏", "Extracting justified schedule that respects constraints");

    // Sort clocks topologically
    let sorted_clocks = extractor.sort_clocks_topologically();
    if sorted_clocks.is_empty() {
        return Err("No clocks found to schedule".to_string());
    }

    // Get earliest feasible times with a forward pass
    let mut earliest_schedule = HashMap::new();
    extractor.forward_pass(&sorted_clocks, &mut earliest_schedule);

    // Get latest feasible times with a backward pass
    let mut latest_schedule = HashMap::new();
    extractor.backward_pass(&sorted_clocks, &mut latest_schedule);

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

    extractor.debug_print("🌐", &format!(
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
            extractor.debug_print("🏁", &format!(
                "First clock {} at earliest possible time {}",
                clock_id, justified_time
            ));
        } else if i == sorted_clocks.len() - 1 {
            justified_time = latest as i32;
            extractor.debug_print("🏁", &format!(
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
                extractor.debug_print("📍", &format!(
                    "Clock {} target {} pushed forward to {} (its earliest feasible time)",
                    clock_id, target_time as i32, earliest as i32
                ));
                earliest as i32
            } else if target_time > latest {
                extractor.debug_print("📍", &format!(
                    "Clock {} target {} pulled back to {} (its latest feasible time)",
                    clock_id, target_time as i32, latest as i32
                ));
                latest as i32
            } else {
                extractor.debug_print("📍", &format!(
                    "Clock {} at {}/{} of span: {}",
                    clock_id, i, sorted_clocks.len()-1, target_time as i32
                ));
                target_time as i32
            };
        }

        justified_schedule.insert(clock_id.clone(), justified_time);
        extractor.debug_set_time(clock_id, justified_time);
    }

    extractor.debug_print("🔄", "Verifying and fixing any constraint violations");
    extractor.fix_constraint_violations(&sorted_clocks, &mut justified_schedule);

    Ok(justified_schedule)
}

// Maximum Spread schedule that respects constraints
pub fn extract_max_spread_with_constraints(extractor: &ScheduleExtractor) -> Result<HashMap<String, i32>, String> {
    extractor.debug_print("↔️", "Extracting maximum spread schedule that respects constraints");

    // Sort clocks topologically
    let sorted_clocks = extractor.sort_clocks_topologically();
    if sorted_clocks.is_empty() {
        return Err("No clocks found to schedule".to_string());
    }

    // Get earliest and latest feasible times
    let mut earliest_schedule = HashMap::new();
    extractor.forward_pass(&sorted_clocks, &mut earliest_schedule);

    let mut latest_schedule = HashMap::new();
    extractor.backward_pass(&sorted_clocks, &mut latest_schedule);

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

    extractor.debug_print("🌐", &format!(
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
            extractor.debug_print("📍", &format!(
                "Clock {} ideal {} pushed forward to {} (its earliest feasible time)",
                clock_id, ideal_time, earliest
            ));
            earliest
        } else if ideal_time > latest {
            extractor.debug_print("📍", &format!(
                "Clock {} ideal {} pulled back to {} (its latest feasible time)",
                clock_id, ideal_time, latest
            ));
            latest
        } else {
            extractor.debug_print("📍", &format!(
                "Clock {} at {}/{} of span: {}",
                clock_id, i, sorted_clocks.len()-1, ideal_time
            ));
            ideal_time
        };

        spread_schedule.insert(clock_id.clone(), spread_time);
        extractor.debug_set_time(clock_id, spread_time);
    }

    extractor.debug_print("🔄", "Verifying and fixing any constraint violations");
    extractor.fix_constraint_violations(&sorted_clocks, &mut spread_schedule);

    Ok(spread_schedule)
}