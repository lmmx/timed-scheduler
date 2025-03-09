use generate_schedule::example;

fn main() {
    match example() {
        Ok(_) => println!("Successfully generated schedule!"),
        Err(e) => eprintln!("Error: {}", e),
    }
}
