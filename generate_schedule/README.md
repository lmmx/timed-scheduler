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

# Schedule Strategy Command-Line Option Guide

I've added the ability to select different scheduling strategies through command-line options. This
allows you to easily experiment with different approaches to schedule extraction without modifying
the code.

### Basic Usage

To run the scheduler with a specific strategy:

```bash
cargo run -- --strategy justified
# Or using the short form
cargo run -- -s justified
```

### Available Strategies

The following strategies are available:

1. **earliest** - Schedule all events at their earliest possible time
2. **latest** - Schedule all events at their latest possible time
3. **centered** - Schedule all events in the middle of their feasible range (default)
4. **justified** - Distribute events evenly across the feasible time span
5. **spread** (or **maximumspread**) - Maximize the spacing between events

### Combining with Debug

You can combine the strategy flag with the debug flag:

```bash
cargo run -- --strategy earliest --debug
```

### Getting Help

To display usage information:

```bash
cargo run -- --help
# Or
cargo run -- -h
```

## Example Command Lines

```bash
# Use earliest strategy with debug output
cargo run -- -s earliest -d

# Use justified strategy
cargo run -- --strategy justified

# Use latest strategy with debug output
cargo run -- --strategy latest --debug

# Use spread strategy
cargo run -- -s spread
```

## Default Behavior

If no strategy is specified, the program will default to using the `Centered` strategy, which places
each event in the middle of its feasible time range.

## Error Handling

- If an unknown strategy is specified, the program will warn you and default to the `Centered`
  strategy
- Suggestions to run with `--help` are provided when errors occur
- If a strategy is specified without a value, it will default to `Centered`

## Important Note

When using the strategy flag with debug output, you'll now see the schedule printed twice:
1. First in the debug output after extraction (sorted by time)
2. Then in the final formatted output (also sorted by time)

This provides a consistent view of the schedule at different stages of the process.

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
