mod domain;
mod parse;
mod cli;

use crate::cli::{ScheduleStrategy, parse_config_from_args};
use crate::domain::{ClockVar, ConstraintType, ConstraintRef, c2str};
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
            "[\"08:00\", \"12:00-13:00\", \"19:00\"]",
            "null",
        ],
    ];

    let entities = parse_from_table(table_data)?;

    let mut category_map = HashMap::new();
    for e in &entities {
        category_map.entry(e.category.clone())
            .or_insert_with(HashSet::new)
            .insert(e.name.clone());
    }

    // Build clock variables, clamped to the day window
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
            clock_map.insert(cname, ClockVar {
                entity_name: e.name.clone(),
                instance: i+1,
                var
            });
        }
    }

    let mut constraints = Vec::new();
    fn add_dbg(desc: &str, c: Constraint, vec: &mut Vec<Constraint>) {
        println!("DEBUG => {desc}");
        vec.push(c);
    }

    // Build an entity->Vec<ClockVar> map
    let mut entity_clocks: HashMap<String, Vec<ClockVar>> = HashMap::new();
    for cv in clock_map.values() {
        entity_clocks.entry(cv.entity_name.clone())
            .or_default()
            .push(cv.clone());
    }
    for list in entity_clocks.values_mut() {
        list.sort_by_key(|c| c.instance);
    }

    // Resolve references by name or category
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

    // Apply existing "apart/before/after" logic
    for e in &entities {
        let eclocks = if let Some(list) = entity_clocks.get(&e.name) {
            list
        } else {
            continue;
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

        // a) apply "apart" to consecutive
        for tv in apart_intervals {
            for w in eclocks.windows(2) {
                let c1 = &w[0];
                let c2 = &w[1];
                let desc = format!("(Apart) {} - {} >= {}", c2str(c2), c2str(c1), tv);
                add_dbg(&desc, constraint!( c2.var - c1.var >= tv ), &mut constraints);
            }
        }

        // b) apply "apart_from" => big-M
        for (tv, refname) in apart_from_list {
            let rvars = resolve_ref(&refname);
            for c_e in eclocks {
                for c_r in &rvars {
                    let b = builder.add(variable().binary());
                    let d1 = format!("(ApartFrom) {} - {} >= {} - bigM*(1-b)",
                        c2str(c_r), c2str(c_e), tv);
                    add_dbg(&d1,
                        constraint!( c_r.var - c_e.var >= tv - big_m*(1.0 - b)),
                        &mut constraints
                    );
                    let d2 = format!("(ApartFrom) {} - {} >= {} - bigM*b",
                        c2str(c_e), c2str(c_r), tv);
                    add_dbg(&d2,
                        constraint!( c_e.var - c_r.var >= tv - big_m*b),
                        &mut constraints
                    );
                }
            }
        }

        // c) merges of "before & after"
        for (rname,(maybe_b, maybe_a)) in ba_map {
            let rvars = resolve_ref(&rname);
            match (maybe_b, maybe_a) {
                (Some(bv), Some(av)) => {
                    for c_e in eclocks {
                        for c_r in &rvars {
                            let b = builder.add(variable().binary());
                            let d1 = format!("(Before|After) {} - {} >= {} - M*(1-b)",
                                c2str(c_r), c2str(c_e), bv);
                            add_dbg(&d1,
                                constraint!( c_r.var - c_e.var >= bv - big_m*(1.0 - b)),
                                &mut constraints
                            );
                            let d2 = format!("(Before|After) {} - {} >= {} - M*b",
                                c2str(c_e), c2str(c_r), av);
                            add_dbg(&d2,
                                constraint!( c_e.var - c_r.var >= av - big_m*b),
                                &mut constraints
                            );
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

    // Objective
    let mut sum_expr = Expression::from(0);
    for cv in clock_map.values() {
        sum_expr += cv.var;
    }

    let mut problem = match config.strategy {
        ScheduleStrategy::Earliest => builder.minimise(sum_expr).using(default_solver),
        ScheduleStrategy::Latest   => builder.maximise(sum_expr).using(default_solver),
    };

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
    schedule.sort_by(|a,b| a.2.partial_cmp(&b.2).unwrap());

    println!("--- Final schedule ({:?}) ---", config.strategy);
    for (cid, ename, t) in schedule {
        let hh = (t/60.0).floor() as i32;
        let mm = (t%60.0).round() as i32;
        println!("  {cid} ({ename}): {hh:02}:{mm:02}");
    }

    Ok(())
}
