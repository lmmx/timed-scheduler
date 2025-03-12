use good_lp::{
    variables, variable, constraint, default_solver, SolverModel, 
    Solution,
};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // 1) Create a new set of variables:
    //    "builder" is your handle to define & store them.
    let mut builder = variables!();

    // 2) Add two integer variables, x and y:
    let x = builder.add(variable().integer().min(0).max(10));
    let y = builder.add(variable().integer().min(0).max(10));

    // 3) Create an objective expression:
    //    good_lp needs us to build an Expression, not a float.
    //    We can combine variables with normal arithmetic:
    let objective = x + y; // Minimizing (x + y).

    // 4) Build the problem using "minimise(...).using(...)".
    //    We can chain constraints via ".with(...)"
    let problem = builder
        .minimise(objective)
        .using(default_solver)  // pick a solver (Cbc by default)
        .with(constraint!( x + y >= 12 )) // for example
        .with(constraint!( x >= 3 ))      // x≥3
        .with(constraint!( y >= 4 ));     // y≥4

    // 5) Solve the problem
    let solution = problem.solve()?;

    // 6) Extract the solution values:
    let x_val = solution.value(x);
    let y_val = solution.value(y);

    println!("Solution found:");
    println!("  x = {}", x_val);
    println!("  y = {}", y_val);

    // For instance, if x + y >= 12, x≥3, y≥4, and we're minimizing x+y,
    // The solver might pick x=3, y=9 or x=4, y=8, etc. depending on constraints.

    Ok(())
}
