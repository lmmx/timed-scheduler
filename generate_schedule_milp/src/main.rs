use generate_schedule::{example, ScheduleStrategy};
use std::env;
use std::process;

fn main() {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    // Check for help flag first
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_usage();
        return;
    }

    // Parse strategy from args
    let strategy = parse_strategy_from_args();

    match example(strategy) {
        Ok(_) => println!("Successfully generated schedule!"),
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}

/// Parses command line arguments to determine the schedule strategy
fn parse_strategy_from_args() -> ScheduleStrategy {
    let args: Vec<String> = env::args().collect();

    for (i, arg) in args.iter().enumerate() {
        if arg == "--strategy" || arg == "-s" {
            if i + 1 < args.len() {
                return match args[i + 1].to_lowercase().as_str() {
                    "earliest" => ScheduleStrategy::Earliest,
                    "latest" => ScheduleStrategy::Latest,
                    "centered" => ScheduleStrategy::Centered,
                    "justified" => ScheduleStrategy::Justified,
                    "spread" | "maximumspread" => ScheduleStrategy::MaximumSpread,
                    _ => {
                        eprintln!(
                            "Warning: Unknown strategy '{}', defaulting to Centered",
                            args[i + 1]
                        );
                        eprintln!("Run with --help for a list of available strategies");
                        ScheduleStrategy::Centered
                    }
                };
            }
        }
    }

    // Default to Centered if no strategy specified
    ScheduleStrategy::Centered
}

/// Prints the command-line usage information
fn print_usage() {
    println!("Schedule Generator - Command Line Options\n");
    println!("USAGE:");
    println!("    generate_schedule [OPTIONS]\n");
    println!("OPTIONS:");
    println!("    -h, --help                  Display this help message");
    println!("    -d, --debug                 Enable debug output");
    println!("    -s, --strategy STRATEGY     Set the schedule extraction strategy");
    println!("\nSTRATEGIES:");
    println!("    earliest       Schedule all events at their earliest possible time");
    println!("    latest         Schedule all events at their latest possible time");
    println!(
        "    centered       Schedule all events in the middle of their feasible range (default)"
    );
    println!("    justified      Distribute events evenly across the feasible time span");
    println!("    spread         Maximize the spacing between events (similar to justified)");
    println!("\nEXAMPLES:");
    println!("    generate_schedule --strategy earliest --debug");
    println!("    generate_schedule -s justified");
}
