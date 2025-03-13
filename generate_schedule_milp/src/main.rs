mod domain;
mod parse;
mod cli;

use crate::cli::{ScheduleStrategy, parse_config_from_args};
use crate::domain::{
    ClockVar, ConstraintType, ConstraintRef, c2str,
    WindowSpec, Entity, // needed to match on WindowSpec
};
use crate::parse::parse_from_table;

use good_lp::{
    variables, variable, constraint, default_solver,
    SolverModel, Solution, Expression, Constraint, Variable
};
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::time::Instant;

// Custom structure to track penalty variables for better reporting
struct PenaltyVar {
    entity_name: String,
    instance: usize,
    var: Variable,
}

// Structure to capture window information for better reporting
#[derive(Debug, Clone)]
struct WindowInfo {
    time_desc: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let start_time = Instant::now();

    let config = parse_config_from_args();
    println!("Using day window: {}..{} (in minutes)", config.day_start_minutes, config.day_end_minutes);
    println!("Strategy: {:?}", config.strategy);

    // Sample table data
    let table_data = vec![
        vec![
            "Entity",
            "Category",
            "Unit",
            "Amount",
            "Split",
            "Frequency",
            "Constraints",
            "Windows",
            "Note",
        ],
        vec![
            "Antepsin",
            "med",
            "tablet",
            "null",
            "3",
            "3x daily",
            "[\"≥6h apart\", \"≥1h before food\", \"≥2h after food\"]",
            "[]", // no windows
            "in 1tsp water",
        ],
        vec![
            "Gabapentin",
            "med",
            "ml",
            "1.8",
            "null",
            "2x daily",
            "[\"≥8h apart\"]",
            "[]",
            "null",
        ],
        vec![
            "Pardale",
            "med",
            "tablet",
            "null",
            "2",
            "2x daily",
            "[\"≥8h apart\"]",
            "[]",
            "null",
        ],
        vec![
            "Pro-Kolin",
            "med",
            "ml",
            "3.0",
            "null",
            "2x daily",
            "[]",
            "[]",
            "with food",
        ],
        vec![
            "Chicken and rice",
            "food",
            "meal",
            "null",
            "null",
            "2x daily",
            "[]",               // no 'apart' constraints
            "[\"08:00\", \"18:00-20:00\"]", // has 1 anchor & 1 range
            "some note",
        ],
    ];

    // Parse table data
    let entities = parse_from_table(table_data)?;

    // Create a map of window info for better reporting
    let entity_windows = create_window_info_map(&entities);

    // Build category->entities map
    let mut category_map = HashMap::new();
    for e in &entities {
        category_map.entry(e.category.clone())
            .or_insert_with(HashSet::new)
            .insert(e.name.clone());
    }

    // Create variables for each entity instance, within [start..end]
    let mut builder = variables!();
    let mut clock_map = HashMap::new();
    for e in &entities {
        let count = e.frequency.instances_per_day();
        for i in 0..count {
            let cname = format!("{}_{}", e.name, i+1);
            let var = builder
                .add(variable()
                    .integer()
                    .min(config.day_start_minutes as f64)
                    .max(config.day_end_minutes as f64)
                );
            clock_map.insert(
                cname,
                ClockVar {
                    entity_name: e.name.clone(),
                    instance: i+1,
                    var,
                },
            );
        }
    }

    // We collect constraints here
    let mut constraints = Vec::new();

    // More concise debug function with toggle
    let debug_enabled = true;
    fn add_constraint(desc: &str, c: Constraint, vec: &mut Vec<Constraint>, debug: bool) {
        if debug {
            println!("DEBUG => {desc}");
        }
        vec.push(c);
    }

    // Make a map: entity -> [its clockvars]
    let mut entity_clocks: HashMap<String, Vec<ClockVar>> = HashMap::new();
    for cv in clock_map.values() {
        entity_clocks.entry(cv.entity_name.clone())
            .or_default()
            .push(cv.clone());
    }
    for list in entity_clocks.values_mut() {
        list.sort_by_key(|c| c.instance);
    }

    // Helper to resolve references: either an entity name or a category
    let resolve_ref = |rstr: &str| -> Vec<ClockVar> {
        let mut out = Vec::new();
        for e in &entities {
            if e.name.eq_ignore_ascii_case(rstr) {
                if let Some(cl) = entity_clocks.get(&e.name) {
                    out.extend(cl.clone());
                }
            }
        }
        if !out.is_empty() {
            return out;
        }
        if let Some(nameset) = category_map.get(rstr) {
            for nm in nameset {
                if let Some(cl) = entity_clocks.get(nm) {
                    out.extend(cl.clone());
                }
            }
        }
        out
    };

    let big_m = 1440.0;

    // (1) Apply "apart/before/after" constraints
    for e in &entities {
        let eclocks = match entity_clocks.get(&e.name) {
            Some(list) => list,
            None => continue,
        };

        let mut ba_map: HashMap<String, (Option<f64>, Option<f64>)> = HashMap::new();
        let mut apart_intervals = Vec::new();
        let mut apart_from_list = Vec::new();

        for cexpr in &e.constraints {
            let tv_min = (cexpr.time_hours as f64) * 60.0;
            match cexpr.ctype {
                ConstraintType::Apart => {
                    apart_intervals.push(tv_min);
                }
                ConstraintType::ApartFrom => {
                    if let ConstraintRef::Unresolved(r) = &cexpr.cref {
                        apart_from_list.push((tv_min, r.clone()));
                    }
                }
                ConstraintType::Before => {
                    if let ConstraintRef::Unresolved(r) = &cexpr.cref {
                        let ent = ba_map.entry(r.clone()).or_insert((None, None));
                        ent.0 = Some(tv_min);
                    }
                }
                ConstraintType::After => {
                    if let ConstraintRef::Unresolved(r) = &cexpr.cref {
                        let ent = ba_map.entry(r.clone()).or_insert((None, None));
                        ent.1 = Some(tv_min);
                    }
                }
            }
        }

        // (a) "apart" for consecutive instances
        for tv in apart_intervals {
            for w in eclocks.windows(2) {
                let c1 = &w[0];
                let c2 = &w[1];
                let desc = format!("(Apart) {} - {} >= {}", c2str(c2), c2str(c1), tv);
                add_constraint(&desc, constraint!(c2.var - c1.var >= tv), &mut constraints, debug_enabled);
            }
        }

        // (b) "apart_from" => big-M disjunction
        for (tv, refname) in apart_from_list {
            let rvars = resolve_ref(&refname);
            for c_e in eclocks {
                for c_r in &rvars {
                    let b = builder.add(variable().binary());
                    let d1 = format!("(ApartFrom) {} - {} >= {} - bigM*(1-b)",
                        c2str(c_r), c2str(&c_e), tv);
                    add_constraint(&d1,
                        constraint!(c_r.var - c_e.var >= tv - big_m*(1.0 - b)),
                        &mut constraints,
                        debug_enabled
                    );

                    let d2 = format!("(ApartFrom) {} - {} >= {} - bigM*b",
                        c2str(&c_e), c2str(c_r), tv);
                    add_constraint(&d2,
                        constraint!(c_e.var - c_r.var >= tv - big_m*b),
                        &mut constraints,
                        debug_enabled
                    );
                }
            }
        }

        // (c) merges of "before & after"
        for (rname, (maybe_b, maybe_a)) in ba_map {
            let rvars = resolve_ref(&rname);
            match (maybe_b, maybe_a) {
                (Some(bv), Some(av)) => {
                    // "≥bv before" OR "≥av after" disjunction
                    for c_e in eclocks {
                        for c_r in &rvars {
                            let b = builder.add(variable().binary());
                            let d1 = format!("(Before|After) {} - {} >= {} - M*(1-b)",
                                c2str(c_r), c2str(&c_e), bv);
                            add_constraint(&d1,
                                constraint!(c_r.var - c_e.var >= bv - big_m*(1.0 - b)),
                                &mut constraints,
                                debug_enabled
                            );

                            let d2 = format!("(Before|After) {} - {} >= {} - M*b",
                                c2str(&c_e), c2str(c_r), av);
                            add_constraint(&d2,
                                constraint!(c_e.var - c_r.var >= av - big_m*b),
                                &mut constraints,
                                debug_enabled
                            );
                        }
                    }
                }
                (Some(bv), None) => {
                    // only "before"
                    for c_e in eclocks {
                        for c_r in &rvars {
                            let d = format!("(Before) {} - {} >= {}", c2str(c_r), c2str(c_e), bv);
                            add_constraint(&d,
                                constraint!(c_r.var - c_e.var >= bv),
                                &mut constraints,
                                debug_enabled
                            );
                        }
                    }
                }
                (None, Some(av)) => {
                    // only "after"
                    for c_e in eclocks {
                        for c_r in &rvars {
                            let d = format!("(After) {} - {} >= {}", c2str(c_e), c2str(c_r), av);
                            add_constraint(&d,
                                constraint!(c_e.var - c_r.var >= av),
                                &mut constraints,
                                debug_enabled
                            );
                        }
                    }
                }
                (None, None) => {}
            }
        }
    }

    // (2) SOFT penalty for window preferences
    // Use a moderate alpha that balances earliest/latest with window preferences
    let alpha = 0.3;

    println!("\n--- Creating soft window penalty constraints (α = {}) ---", alpha);

    // Track penalty variables for better reporting
    let mut penalty_vars: Vec<PenaltyVar> = Vec::new();

    // Track which windows are used by which instances
    let mut window_usage_vars: HashMap<String, HashMap<(usize, usize), Variable>> = HashMap::new();

    for e in &entities {
        // Skip entities with no windows - they won't have penalties
        if e.windows.is_empty() {
            continue;
        }

        println!("Entity '{}': {} windows defined", e.name, e.windows.len());

        // Get clock variables for this entity
        let eclocks = match entity_clocks.get(&e.name) {
            Some(list) => list,
            None => continue,
        };

        // If we have multiple instances and multiple windows, track window usage
        let track_window_usage = eclocks.len() > 1 && e.windows.len() > 1;
        let mut instance_window_vars = HashMap::new();

        // Process each clock variable (instance) for this entity
        for cv in eclocks {
            // Create a penalty variable p_i for this instance
            let p_i = builder.add(variable().min(0.0));

            // Store penalty info for reporting
            penalty_vars.push(PenaltyVar {
                entity_name: e.name.clone(),
                instance: cv.instance,
                var: p_i,
            });

            // Create one distance variable for each window
            for (w_idx, wspec) in e.windows.iter().enumerate() {
                let dist_iw = builder.add(variable().min(0.0));

                // For window distribution tracking
                if track_window_usage {
                    // Create binary variable indicating if this instance uses this window
                    let window_use_var = builder.add(variable().binary());
                    instance_window_vars.insert((cv.instance, w_idx), window_use_var);

                    // Define "using a window" as being within 30 minutes of it
                    let use_threshold = 30.0;

                    // If dist_iw <= use_threshold then window_use_var = 1
                    // Using big-M: dist_iw <= use_threshold + M*(1-window_use_var)
                    add_constraint(
                        &format!("(WinUse) {}_{} uses win{} if dist <= {}",
                                 e.name, cv.instance, w_idx, use_threshold),
                        constraint!(dist_iw <= use_threshold + big_m*(1.0 - window_use_var)),
                        &mut constraints,
                        debug_enabled
                    );

                    // If dist_iw > use_threshold then window_use_var = 0
                    // Using big-M: dist_iw >= use_threshold - M*window_use_var
                    add_constraint(
                        &format!("(WinUse) {}_{} doesn't use win{} if dist > {}",
                                 e.name, cv.instance, w_idx, use_threshold),
                        constraint!(dist_iw >= use_threshold - big_m*window_use_var),
                        &mut constraints,
                        debug_enabled
                    );
                }

                match wspec {
                    WindowSpec::Anchor(a) => {
                        // For anchors: |t_i - a| represented with two constraints
                        // dist_iw >= t_i - a
                        add_constraint(
                            &format!("(Win+) dist_{}_w{} >= {} - {}", cv.instance, w_idx, c2str(cv), a),
                            constraint!(dist_iw >= cv.var - (*a as f64)),
                            &mut constraints,
                            debug_enabled
                        );

                        // dist_iw >= a - t_i
                        add_constraint(
                            &format!("(Win-) dist_{}_w{} >= {} - {}", cv.instance, w_idx, a, c2str(cv)),
                            constraint!(dist_iw >= (*a as f64) - cv.var),
                            &mut constraints,
                            debug_enabled
                        );
                    },
                    WindowSpec::Range(start, end) => {
                        // For ranges: 0 if inside, distance to closest edge if outside
                        // dist_iw >= start - t_i (if t_i < start)
                        add_constraint(
                            &format!("(WinS) dist_{}_w{} >= {} - {}", cv.instance, w_idx, start, c2str(cv)),
                            constraint!(dist_iw >= (*start as f64) - cv.var),
                            &mut constraints,
                            debug_enabled
                        );

                        // dist_iw >= t_i - end (if t_i > end)
                        add_constraint(
                            &format!("(WinE) dist_{}_w{} >= {} - {}", cv.instance, w_idx, c2str(cv), end),
                            constraint!(dist_iw >= cv.var - (*end as f64)),
                            &mut constraints,
                            debug_enabled
                        );
                    }
                }

                // p_i <= dist_iw => p_i will be minimum distance to any window
                add_constraint(
                    &format!("(Win) p_{} <= dist_{}_w{}", cv.instance, cv.instance, w_idx),
                    constraint!(p_i <= dist_iw),
                    &mut constraints,
                    debug_enabled
                );
            }
        }

        // If we're tracking window usage for this entity, save the variables
        if track_window_usage {
            window_usage_vars.insert(e.name.clone(), instance_window_vars);
        }
    }

    // (3) Window distribution constraints
    // Ensure instances of the same entity use different windows when possible
    println!("\n--- Adding window distribution constraints ---");

    for (ename, instance_window_map) in &window_usage_vars {
        let eclocks = entity_clocks.get(ename).unwrap();
        let window_count = entities.iter()
            .find(|e| &e.name == ename)
            .map(|e| e.windows.len())
            .unwrap_or(0);

        println!("Entity '{}': ensuring distribution across {} windows", ename, window_count);

        // Each instance must use exactly one window
        for cv in eclocks {
            let mut sum_expr = Expression::from(0.0);
            for w_idx in 0..window_count {
                if let Some(&use_var) = instance_window_map.get(&(cv.instance, w_idx)) {
                    sum_expr += use_var;
                }
            }

            add_constraint(
                &format!("(Dist) {}_instance{} must use exactly one window", ename, cv.instance),
                constraint!(sum_expr == 1.0),
                &mut constraints,
                debug_enabled
            );
        }

        // Each window can be used at most once
        // (this forces distribution across windows)
        for w_idx in 0..window_count {
            let mut sum_expr = Expression::from(0.0);
            for cv in eclocks {
                if let Some(&use_var) = instance_window_map.get(&(cv.instance, w_idx)) {
                    sum_expr += use_var;
                }
            }

            add_constraint(
                &format!("(Dist) {}_window{} can be used at most once", ename, w_idx),
                constraint!(sum_expr <= 1.0),
                &mut constraints,
                debug_enabled
            );
        }
    }

    // (4) Build objective:
    // For earliest => minimize(sum(t_i) + alpha * sum(p_i))
    // For latest   => maximize(sum(t_i) - alpha * sum(p_i))
    //               = minimize(-sum(t_i) + alpha * sum(p_i))

    // Sum of all time variables
    let mut sum_expr = Expression::from(0.0);
    for cv in clock_map.values() {
        sum_expr += cv.var;
    }

    // Sum of all penalty variables
    let mut penalty_expr = Expression::from(0.0);
    for p in &penalty_vars {
        penalty_expr += p.var;
    }

    // Add all constraints to the problem
    println!("\nSolving problem with {} constraints...", constraints.len());

    let mut problem = match config.strategy {
        ScheduleStrategy::Earliest => {
            println!("\nObjective: minimize(sum(t_i) + {} * sum(p_i))", alpha);
            builder.minimise(sum_expr + alpha * penalty_expr)
                   .using(default_solver)
        }
        ScheduleStrategy::Latest => {
            println!("\nObjective: maximize(sum(t_i) - {} * sum(p_i))", alpha);
            // Equivalent to minimize(-sum_expr + alpha * penalty_expr)
            builder.minimise(Expression::from(0.0) - sum_expr + alpha * penalty_expr)
                   .using(default_solver)
        }
    };

    // 1) Take the length before consuming constraints.
    let constraint_count = constraints.len();

    // 2) Now actually consume the constraints.
    for c in constraints {
        problem = problem.with(c);
    }
    let solve_start = Instant::now();

    let sol = match problem.solve() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Solver error => {e}");
            return Err(format!("Solve error: {e}").into());
        }
    };

    let solve_time = solve_start.elapsed();
    println!("Problem solved in {:.2?}", solve_time);

    // Extract solution and organize for display
    let mut schedule = Vec::new();
    for (cid, cv) in &clock_map {
        let val = sol.value(cv.var);
        schedule.push((cid.clone(), cv.entity_name.clone(), cv.instance, val));
    }
    schedule.sort_by(|a, b| a.3.partial_cmp(&b.3).unwrap());

    // Display the final schedule with formatting
    println!("\n┌─────────────────────────────────────────────┐");
    println!("│           FINAL SCHEDULE ({:?})          │", config.strategy);
    println!("├─────────────────────────────────────────────┤");
    println!("│ Time     | Instance                | Entity │");
    println!("├──────────┼─────────────────────────┼────────┤");

    for (cid, ename, _instance, t) in &schedule {
        let hh = (t / 60.0).floor() as i32;
        let mm = (t % 60.0).round() as i32;
        println!("│ {:02}:{:02}    | {:<23} | {:<6} │",
                 hh, mm, cid, ename);
    }
    println!("└─────────────────────────────────────────────┘");

    // Display window usage information
    if !window_usage_vars.is_empty() {
        println!("\n┌─────────────────────────────────────────────┐");
        println!("│           WINDOW USAGE REPORT              │");
        println!("├─────────────────────────────────────────────┤");
        println!("│ Entity     | Window          | Used By      │");
        println!("├────────────┼─────────────────┼──────────────┤");

        for (ename, instance_window_map) in &window_usage_vars {
            let e = entities.iter().find(|e| e.name == *ename).unwrap();

            for w_idx in 0..e.windows.len() {
                let window_desc = match &entity_windows.get(ename).unwrap()[w_idx].time_desc {
                    desc => desc.clone(),
                };

                let mut users = Vec::new();
                for (instance, idx) in instance_window_map.keys() {
                    if *idx == w_idx && sol.value(instance_window_map[&(*instance, *idx)]) > 0.5 {
                        users.push(format!("#{}", instance));
                    }
                }

                let usage = if users.is_empty() { "None".to_string() } else { users.join(", ") };

                println!("│ {:<10} | {:<15} | {:<12} │",
                         ename, window_desc, usage);
            }
            println!("├────────────┼─────────────────┼──────────────┤");
        }
        println!("└─────────────────────────────────────────────┘");
    }

    // Calculate and display window penalties
    if !penalty_vars.is_empty() {
        println!("\n┌───────────────────────────────────────────────────────┐");
        println!("│                WINDOW ADHERENCE REPORT                │");
        println!("├───────────────────────────────────────────────────────┤");
        println!("│ Entity     | Instance | Deviation | Preferred Windows │");
        println!("├────────────┼──────────┼───────────┼───────────────────┤");

        let mut total_penalty = 0.0;

        for p in &penalty_vars {
            let p_val = sol.value(p.var);
            total_penalty += p_val;

            // Find the actual time for this entity/instance
            let instance_time = schedule.iter()
                .find(|(_, ename, inst, _)| ename == &p.entity_name && inst == &p.instance)
                .map(|(_, _, _, t)| *t)
                .unwrap_or(0.0);

            let hh = (instance_time / 60.0).floor() as i32;
            let mm = (instance_time % 60.0).round() as i32;

            // Get window descriptions for this entity
            let window_descriptions = match entity_windows.get(&p.entity_name) {
                Some(windows) => windows.iter()
                    .map(|w| w.time_desc.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
                None => "None".to_string(),
            };

            let deviation_display = if p_val < 0.001 {
                "On target".to_string()
            } else {
                format!("{:.1} min", p_val)
            };

            println!("│ {:<10} | {:<8} | {:<9} | {:<17} │",
                     p.entity_name,
                     format!("#{} ({:02}:{:02})", p.instance, hh, mm),
                     deviation_display,
                     window_descriptions);
        }

        println!("├────────────┴──────────┴───────────┴───────────────────┤");
        println!("│ Total penalty: {:<39.1} │", total_penalty);
        println!("└───────────────────────────────────────────────────────┘");
    }

    // Display performance metrics
    let total_time = start_time.elapsed();
    println!("\nTotal runtime: {:.2?}", total_time);
    println!("Number of entities: {}", entities.len());
    println!("Number of scheduled instances: {}", schedule.len());
    println!("Number of constraints: {}", constraint_count);

    Ok(())
}

// Helper function to create window descriptions for better reporting
fn create_window_info_map(entities: &[Entity]) -> HashMap<String, Vec<WindowInfo>> {
    let mut result = HashMap::new();

    for e in entities {
        if e.windows.is_empty() {
            continue;
        }

        let mut windows = Vec::new();

        for w in &e.windows {
            match w {
                WindowSpec::Anchor(a) => {
                    let hh = a / 60;
                    let mm = a % 60;
                    windows.push(WindowInfo {
                        time_desc: format!("{:02}:{:02}", hh, mm),
                    });
                },
                WindowSpec::Range(start, end) => {
                    let start_hh = start / 60;
                    let start_mm = start % 60;
                    let end_hh = end / 60;
                    let end_mm = end % 60;
                    windows.push(WindowInfo {
                        time_desc: format!("{:02}:{:02}-{:02}:{:02}",
                                          start_hh, start_mm, end_hh, end_mm),
                    });
                },
            }
        }

        result.insert(e.name.clone(), windows);
    }

    result
}
