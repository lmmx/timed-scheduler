# Time Constraint Scheduling Library

This library provides a domain-specific language (DSL) for specifying time-based constraints and a compiler that generates feasible schedules based on those constraints.

## Project Structure

The codebase is organized into modules:

```
src/
├── lib.rs               # Main library file with re-exports
├── types/               # Core data types
├── compiler/            # Schedule generation logic
└── parser/              # Input format parsers
```

## Key Components

### Types

- `Frequency`: Defines how often an entity occurs (daily, twice daily, etc.)
- `Entity`: Represents an item to be scheduled with constraints
- `TimeUnit`: Represents time units (minutes, hours)
- `ConstraintExpression`: Represents timing constraints between entities

### Compiler

- `TimeConstraintCompiler`: Converts DSL constraints into a time zone model
- `ClockInfo`: Tracks information about clock variables

### Parser

- `parse_from_table`: Parses entity definitions from a tabular format

## Example Usage

```rust
use time_constraint_lib::{Entity, TimeConstraintCompiler, parse_from_table};

fn main() -> Result<(), String> {
    // Define entities in tabular format
    let table_data = vec![
        vec![
            "Entity", "Category", "Unit", "Amount", "Split", "Frequency", "Constraints", "Note",
        ],
        vec![
            "Medication A", "medicine", "mg", "10", "null", "2x daily", 
            "[\"≥30m apart from food\"]", "Take with water"
        ],
        vec![
            "Breakfast", "food", "meal", "null", "null", "daily", 
            "[]", "null"
        ],
    ];

    // Parse entities
    let entities = parse_from_table(table_data)?;

    // Create compiler and generate schedule
    let mut compiler = TimeConstraintCompiler::new(entities);
    let zone = compiler.compile()?;

    // Extract a concrete schedule
    let schedule = compiler.extract_schedule()?;

    // Display formatted schedule
    let formatted = compiler.format_schedule(&schedule);
    println!("{}", formatted);

    Ok(())
}
```

## Constraint Syntax

The library supports the following constraint types:

- `≥Xh before Y`: Schedule at least X hours before Y
- `≥Xm after Y`: Schedule at least X minutes after Y
- `≥Xh apart from Y`: Keep separated from Y by at least X hours
- `≥Xm apart`: Keep instances of the same entity separated by at least X minutes

## Dependencies

- `clock_zones`: For zone-based time constraint solving
- `regex`: For parsing constraint expressions
- `serde`: For serialization/deserialization support