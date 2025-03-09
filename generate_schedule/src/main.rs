use generate_schedule::*;

fn main() -> Result<(), String> {
    // Define table data (this would normally come from a file or UI)
    let table_data = vec![
        vec![
            "Entity",
            "Category",
            "Unit",
            "Amount",
            "Split",
            "Frequency",
            "Constraints",
            "Note",
        ],
        vec![
            "Antepsin",
            "med",
            "tablet",
            "null",
            "3",
            "3x daily",
            "[\"≥1h before food\", \"≥2h after food\", \"≥2h apart from med\", \"≥6h apart\"]",
            "in 1tsp water",
        ],
        vec![
            "Gabapentin",
            "med",
            "ml",
            "1.8",
            "null",
            "2x daily",
            "[\"≥8h apart\", \"≥30m before food or med\"]",
            "null",
        ],
        vec![
            "Pardale",
            "med",
            "tablet",
            "null",
            "2",
            "2x daily",
            "[\"≥30m before food or med\", \"≥8h apart\"]",
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
            "with food",
        ],
        vec![
            "Omeprazole",
            "med",
            "capsule",
            "null",
            "null",
            "1x daily",
            "[\"≥30m before food\"]",
            "null",
        ],
        vec![
            "Chicken and rice",
            "food",
            "meal",
            "null",
            "null",
            "2x daily",
            "[]",
            "null",
        ],
    ];

    // Parse the entities from the table data
    let entities = parse_from_table(table_data)?;

    // Create compiler and generate schedule
    let mut compiler = TimeConstraintCompiler::new(entities);
    let zone = compiler.compile()?;

    // Check if feasible
    if zone.is_empty() {
        println!("Schedule is not feasible");
        return Err("Schedule is not feasible".to_string());
    }

    // Extract and display the schedule
    let schedule = compiler.extract_schedule()?;
    let formatted = compiler.format_schedule(&schedule);
    println!("{}", formatted);

    Ok(())
}
