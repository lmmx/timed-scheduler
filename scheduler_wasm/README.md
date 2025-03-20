# WASM Scheduler

A WebAssembly-powered scheduling tool that uses mixed-integer linear programming (MILP) to create optimal schedules based on time constraints. The application allows you to define tasks with specific time windows (either anchor times or time ranges) and generates a feasible schedule that respects all constraints.

---

## Key Features

1. **Simple Constraint System**
   - **Anchor times**: Schedule tasks at specific times (±30 minutes flexibility)
   - **Time ranges**: Schedule tasks within specific start and end times
   - **Automatic conflict detection**: The solver will identify when tasks have contradictory time constraints

2. **Interactive Web UI**
   - Add, edit, and reorder tasks through a user-friendly interface
   - Customize day start and end times
   - Visualize the resulting schedule

3. **WebAssembly Integration**
   - Rust backend for constraint solving using the `good_lp` library
   - WASM bindings for fast in-browser computation
   - No server required - all scheduling happens locally in the browser

---

## Development

This project uses Rust compiled to WebAssembly, with a simple HTML/CSS/JS frontend.

### Prerequisites

Install the WebAssembly target and required tools:

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

### Project Structure

- `scheduler_core/`: Rust library with the core scheduling algorithm
- `scheduler_wasm/`: WASM bindings to expose the scheduler to JavaScript
- `scheduler_wasm/web/`: Frontend web application

### Building

To build the project, run the following from the workspace root:

```bash
# Build the WASM module
cargo build --release --target wasm32-unknown-unknown -p scheduler_wasm

# Generate JavaScript bindings
wasm-bindgen target/wasm32-unknown-unknown/release/scheduler_wasm.wasm \
  --out-dir scheduler_wasm/web --target web
```

### Running Locally

After building, start a local web server:

```bash
cd scheduler_wasm/web
python3 -m http.server 8000
```

Then open your browser to `http://localhost:8000` to use the application.

---

## How It Works

The scheduler solves a constraint satisfaction problem using linear programming:

1. Each task is a variable representing its start time
2. Anchor constraints enforce the task must be scheduled within ±30 minutes of the anchor time
3. Range constraints enforce the task must be scheduled within the given start and end times
4. Day boundaries constrain all tasks to be scheduled within the day window
5. The objective function minimizes the sum of start times (scheduling tasks as early as possible)

When conflicts exist (e.g., anchored tasks outside the day window), the solver reports the schedule is infeasible.

---

## License

MIT License. This project demonstrates a practical application of WebAssembly for constraint solving in the browser.
