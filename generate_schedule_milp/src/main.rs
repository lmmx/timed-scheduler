mod domain;
mod parse;
mod cli;

use crate::cli::{ScheduleStrategy, parse_config_from_args};
use crate::domain::{ClockVar, ConstraintType, ConstraintRef, c2str, WindowSpec};
use crate::parse::parse_from_table;

use good_lp::{
    variables, variable, constraint, default_solver,
    SolverModel, Solution, Expression, Constraint
};
use std::collections::{HashMap, HashSet};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let config = parse_config_from_args();
    println!("Using day window: {}..{} (in minutes)", config.day_start_minutes, config.day_end_minutes);
    println!("Strategy: {:?}", config.strategy);

    // "Best of both worlds" alpha:
    let alpha = 0.2;

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
            "[]",
            "[\"08:00\", \"12:00-13:00\", \"19:00\"]", // has 2 anchors & 1 range
            "null",
        ],
    ];

    let entities = parse_from_table(table_data)?;

    // Build category->entities map
    let mut category_map = HashMap::new();
    for e in &entities {
        category_map.entry(e.category.clone())
            .or_insert_with(HashSet::new)
            .insert(e.name.clone());
    }

    let mut builder = variables!();

    // Build clock variables
    let mut clock_map = HashMap::new();
    for e in &entities {
        let count = e.frequency.instances_per_day();
        for i in 0..count {
            let cname = format!("{}_{}", e.name, i+1);
            // default is float/continuous if we don't call .integer() or .binary()
            let var = builder.add(
                variable()
                    .min(config.day_start_minutes as f64)
                    .max(config.day_end_minutes as f64)
            );
            clock_map.insert(cname, ClockVar {
                entity_name: e.name.clone(),
                instance: i+1,
                var,
            });
        }
    }

    // We'll store constraints in a vector
    let mut constraints = Vec::new();
    fn add_dbg(desc: &str, c: Constraint, vec: &mut Vec<Constraint>) {
        println!("DEBUG => {desc}");
        vec.push(c);
    }

    // entity -> Vec of clock vars
    let mut entity_clocks: HashMap<String, Vec<ClockVar>> = HashMap::new();
    for cv in clock_map.values() {
        entity_clocks.entry(cv.entity_name.clone())
            .or_default()
            .push(cv.clone());
    }
    for list in entity_clocks.values_mut() {
        list.sort_by_key(|c| c.instance);
    }

    // Helper to resolve references by name or category
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

    // 1) apart/before/after constraints
    for e in &entities {
        let eclocks = match entity_clocks.get(&e.name) {
            Some(cl) => cl,
            None => continue,
        };

        let mut ba_map: HashMap<String, (Option<f64>, Option<f64>)> = HashMap::new();
        let mut apart_intervals = Vec::new();
        let mut apart_from_list = Vec::new();

        for cexpr in &e.constraints {
            let tv_min = cexpr.time_hours as f64 * 60.0;
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

        // a) consecutive "apart"
        for tv in apart_intervals {
            for w in eclocks.windows(2) {
                let c1 = &w[0];
                let c2 = &w[1];
                let desc = format!("(Apart) {} - {} >= {}", c2str(c2), c2str(c1), tv);
                add_dbg(&desc, constraint!( c2.var - c1.var >= tv ), &mut constraints);
            }
        }

        // b) "apart_from"
        for (tv, refname) in apart_from_list {
            let rvars = resolve_ref(&refname);
            for c_e in eclocks {
                for c_r in &rvars {
                    let b = builder.add(variable().binary());
                    let d1 = format!("(ApartFrom) {} - {} >= {} - bigM*(1-b)", c2str(c_r), c2str(c_e), tv);
                    add_dbg(&d1, constraint!( c_r.var - c_e.var >= tv - big_m*(1.0 - b)), &mut constraints);
                    let d2 = format!("(ApartFrom) {} - {} >= {} - bigM*b", c2str(c_e), c2str(c_r), tv);
                    add_dbg(&d2, constraint!( c_e.var - c_r.var >= tv - big_m*b ), &mut constraints);
                }
            }
        }

        // c) merges of "before & after"
        for (rname, (maybe_b, maybe_a)) in ba_map {
            let rvars = resolve_ref(&rname);
            match (maybe_b, maybe_a) {
                (Some(bv), Some(av)) => {
                    for c_e in eclocks {
                        for c_r in &rvars {
                            let b = builder.add(variable().binary());
                            let d1 = format!("(Before|After) {} - {} >= {} - M*(1-b)", c2str(c_r), c2str(c_e), bv);
                            add_dbg(&d1, constraint!( c_r.var - c_e.var >= bv - big_m*(1.0 - b) ), &mut constraints);
                            let d2 = format!("(Before|After) {} - {} >= {} - M*b", c2str(c_e), c2str(c_r), av);
                            add_dbg(&d2, constraint!( c_e.var - c_r.var >= av - big_m*b ), &mut constraints);
                        }
                    }
                }
                (Some(bv), None) => {
                    for c_e in eclocks {
                        for c_r in &rvars {
                            let d = format!("(Before) {} - {} >= {}", c2str(c_r), c2str(c_e), bv);
                            add_dbg(&d, constraint!( c_r.var - c_e.var >= bv ), &mut constraints);
                        }
                    }
                }
                (None, Some(av)) => {
                    for c_e in eclocks {
                        for c_r in &rvars {
                            let d = format!("(After) {} - {} >= {}", c2str(c_e), c2str(c_r), av);
                            add_dbg(&d, constraint!( c_e.var - c_r.var >= av ), &mut constraints);
                        }
                    }
                }
                (None, None) => {}
            }
        }
    }

    println!("DEBUG => Using alpha = {alpha} for soft-window penalties.");

    // We'll track penalty variables
    let mut penalty_vars = Vec::new();

    // 2) Soft windows: measure "distance from at least one window"
    for e in &entities {
        if e.windows.is_empty() {
            continue;
        }
        println!("DEBUG => Entity '{}' has {} windows => will penalize distance", e.name, e.windows.len());

        if let Some(clocks) = entity_clocks.get(&e.name) {
            for cv in clocks {
                // This is the penalty var for this clock
                let p_i = builder.add(variable().min(0.0)); 
                penalty_vars.push(p_i);

                // For each window, define a dist_iw var
                // then p_i <= dist_iw
                for (w_idx, wspec) in e.windows.iter().enumerate() {
                    let dist_iw = builder.add(variable().min(0.0));

                    match wspec {
                        WindowSpec::Anchor(a) => {
                            // dist_iw >= var - a
                            let d1 = format!("(SoftAnchor+) dist_{}_{} >= {} - {}", c2str(cv), w_idx, c2str(cv), a);
                            add_dbg(&d1, constraint!( dist_iw >= cv.var - (*a as f64) ), &mut constraints);

                            // dist_iw >= a - var
                            let d2 = format!("(SoftAnchor-) dist_{}_{} >= {} - {}", c2str(cv), w_idx, a, c2str(cv));
                            add_dbg(&d2, constraint!( dist_iw >= (*a as f64) - cv.var ), &mut constraints);
                        }
                        WindowSpec::Range(start, end) => {
                            // dist_iw >= start - var
                            let d1 = format!("(SoftRangeStart) dist_{}_{} >= {} - {}", c2str(cv), w_idx, start, c2str(cv));
                            add_dbg(&d1, constraint!( dist_iw >= (*start as f64) - cv.var ), &mut constraints);

                            // dist_iw >= var - end
                            let d2 = format!("(SoftRangeEnd) dist_{}_{} >= {} - {}", c2str(cv), w_idx, c2str(cv), end);
                            add_dbg(&d2, constraint!( dist_iw >= cv.var - (*end as f64) ), &mut constraints);
                        }
                    }

                    // p_i <= dist_iw
                    let dp = format!("(SoftWinPick) {}_p <= dist_{}_{}", c2str(cv), c2str(cv), w_idx);
                    add_dbg(&dp, constraint!( p_i <= dist_iw ), &mut constraints);
                }
            }
        }
    }

    // 3) Build objective
    let mut sum_expr = Expression::from(0.0);
    for cv in clock_map.values() {
        sum_expr += cv.var;
    }

    let mut penalty_expr = Expression::from(0.0);
    for &p in &penalty_vars {
        penalty_expr += p;
    }

    let mut problem = match config.strategy {
        ScheduleStrategy::Earliest => {
            println!("DEBUG => Objective: minimise sum(t_i) + {alpha} * sum(penalties)");
            builder.minimise(sum_expr + alpha * penalty_expr).using(default_solver)
        }
        ScheduleStrategy::Latest => {
            println!("DEBUG => Objective: minimise -sum(t_i) + {alpha} * sum(penalties) [i.e. max sum(t_i) - alpha*penalty]");
            builder.minimise(Expression::from(0.0) - sum_expr + alpha * penalty_expr)
                   .using(default_solver)
        }
    };

    // Add constraints
    for c in constraints {
        problem = problem.with(c);
    }

    // Solve
    let sol = match problem.solve() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Solver error => {e}");
            return Err(format!("Solve error: {e}").into());
        }
    };

    // Extract schedule
    let mut schedule = Vec::new();
    for (cid, cv) in &clock_map {
        let val = sol.value(cv.var);
        schedule.push((cid.clone(), cv.entity_name.clone(), val));
    }
    schedule.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap());

    println!("--- Final schedule ({:?}) ---", config.strategy);
    for (cid, ename, t) in &schedule {
        let hh = (t / 60.0).floor() as i32;
        let mm = (t % 60.0).round() as i32;
        println!("  {cid} ({ename}): {hh:02}:{mm:02}");
    }

    // Print penalty details
    if !penalty_vars.is_empty() {
        println!("\n--- Window deviation penalties ---");
        let mut total_penalty = 0.0;
        for (i, &p) in penalty_vars.iter().enumerate() {
            let pval = sol.value(p);
            if pval.abs() > 1e-6 {
                println!("  penalty[{i}] = {pval}");
            }
            total_penalty += pval;
        }
        println!("Total penalty => {total_penalty:.2}");
    }

    Ok(())
}
