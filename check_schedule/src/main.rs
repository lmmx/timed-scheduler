use clock_zones::{Clock, Constraint, Dbm, Zone};

fn main() {
    // Let's define our clocks
    let medicine_a = Clock::variable(0);
    let food = Clock::variable(1);

    // Create an unconstrained zone with 2 clocks (medicine_a, food)
    let mut zone: Dbm<i64> = Dbm::new_unconstrained(2);

    // Add constraints:
    println!("🔍 Analyzing medication schedule constraints...");

    // "Medicine A must be taken at least 2 hours apart"
    zone.add_constraint(Constraint::new_ge(medicine_a, 2 * 60)); // 2 hours in minutes
    println!("💊 Medicine A must be taken at least 2 hours apart");

    // "Medicine B must be taken at least 30 mins after food"
    // Let's say med B is the same clock as medicine_a for simplicity
    zone.add_constraint(Constraint::new_diff_ge(medicine_a, food, 30));
    println!("🍽️  Medicine A must be taken at least 30 minutes after food");

    println!("\n📊 Schedule Analysis Results:");
    // Now check if constraints are feasible:
    if zone.is_empty() {
        println!("❌ Constraints conflict! No valid schedule possible.");
    } else {
        println!("✅ Constraints valid! Schedule possible.");

        // Format time nicely with hours and minutes
        let format_time = |minutes: i64| -> String {
            let hours = minutes / 60;
            let mins = minutes % 60;
            if hours > 0 {
                if mins > 0 {
                    format!("{}h {}min", hours, mins)
                } else {
                    format!("{}h", hours)
                }
            } else {
                format!("{}min", mins)
            }
        };

        // Optionally, you could explore optimal timings:
        if let Some(earliest_medicine_a) = zone.get_lower_bound(Clock::variable(0)) {
            println!(
                "💊 Earliest time for Medicine A: {} ({})",
                format_time(earliest_medicine_a),
                earliest_medicine_a
            );
        }
        if let Some(earliest_food_time) = zone.get_lower_bound(food) {
            println!(
                "🍽️  Earliest time for food: {} ({})",
                format_time(earliest_food_time),
                earliest_food_time
            );
        }
    }

    // Symbolically allow time to pass, exploring future possibilities:
    zone.future();

    // Perform checks again after symbolic time advancement:
    println!("\n⏳ After allowing time to pass:");
    if zone.is_empty() {
        println!("❌ No schedule is possible in the future.");
    } else {
        println!("✅ Future schedule possibilities remain feasible.");

        // Show updated bounds after time passes
        if let Some(lower_med_a) = zone.get_lower_bound(medicine_a) {
            println!("💊 Medicine A lower bound: {}", lower_med_a);
        } else {
            println!("💊 Medicine A has no lower bound");
        }

        if let Some(upper_med_a) = zone.get_upper_bound(medicine_a) {
            println!("💊 Medicine A upper bound: {}", upper_med_a);
        } else {
            println!("💊 Medicine A has no upper bound ∞");
        }
    }

    println!("\n📝 Schedule Summary:");
    println!("- Take food first 🍽️");
    println!("- Wait at least 30 minutes ⏱️");
    println!("- Take Medicine A 💊");
    println!("- Wait at least 2 hours before next dose 🕒");
}
