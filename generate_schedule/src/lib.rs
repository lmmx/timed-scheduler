// Main library file with re-exports
use clock_zones::Zone;

mod compiler;
mod parser;
mod types;

// Re-export the main types and functionality
pub use compiler::clock_info::ClockInfo;
pub use compiler::compiler::TimeConstraintCompiler;
pub use parser::table_parser::parse_from_table;
pub use types::constraints::{ConstraintExpression, ConstraintReference, ConstraintType};
pub use types::entity::Entity;
pub use types::frequency::Frequency;
pub use types::time_unit::TimeUnit;

// Example of usage with the provided table data
pub fn example() -> Result<(), String> {
    // This would come from parsing the table
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

    let entities = parse_from_table(table_data)?;

    // Create compiler and generate schedule
    let mut compiler = TimeConstraintCompiler::new(entities);
    let zone = compiler.compile()?;

    // Check if feasible
    if zone.is_empty() {
        println!("Schedule is not feasible");
        return Err("Schedule is not feasible".to_string());
    }

    // Extract a concrete schedule
    let schedule = compiler.extract_schedule()?;

    // Display formatted schedule
    let formatted = compiler.format_schedule(&schedule);
    println!("{}", formatted);

    Ok(())
}
