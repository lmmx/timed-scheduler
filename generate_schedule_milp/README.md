# Timed Scheduler

A small **mixed-integer linear programming (MILP)** approach to scheduling events (e.g. “medications” and “meals”) under various timing rules. It merges constraints like “≥6 h apart,” “≥1 h before [some category],” or “≥2 h after [some category]” into **big‐M disjunctions** when both apply to the same pair, ensuring the solver treats them as **OR** instead of **AND**. This avoids contradictory conditions and yields a feasible schedule if one exists.

---

## Key Features

1. **Constraint Expressions**  
   - **`≥Xh apart`**: Consecutive instances of the same entity are separated by at least X hours.  
   - **`≥Xh before X` / `≥Xh after X`**: One entity must occur at least X hours before/after another.  
   - **`≥Xh apart from X`**: Two different entities must be separated by at least X hours in either direction.

2. **Disjunctive “Before–After” Merge**  
   - If an entity is told “≥1 h before SomeCategory” and also “≥2 h after SomeCategory,” the code automatically creates **one** big‐M constraint for “before OR after,” preventing the contradictory “≥1h before AND ≥2h after.”

3. **Earliest / Latest Objectives**  
   - **Earliest**: Minimizes the sum of event start times, pushing them as early as constraints allow.  
   - **Latest**: Maximizes that sum, pushing them as late as feasible.

4. **Debug Logging**  
   - The code prints lines like `(Before|After) (food_var2) - (med_var1) >= 60 - M*(1-b)` so you can see each constraint in its raw form.

---

## Usage

### 1) Build & Run

```bash
cargo run -- --strategy earliest
```
or
```bash
cargo run -- --strategy latest
```
The solver then prints each constraint and either finds a schedule or reports infeasibility.

### 2) Example Table Data

The sample `main.rs` includes a small table describing entities, frequency, and constraints like `[\"≥6h apart\", \"≥1h before food\", \"≥2h after food\"]`. Each row corresponds to an entity with a daily frequency and a list of timing rules.

### 3) Output

- Debug logs list constraints:  
  ```
  DEBUG => (Before|After) (food_var2) - (med_var1) >= 60 - M*(1-b)
  ...
  ```
- Then a final schedule:  
  ```
  --- Final schedule (Earliest) ---
    SomeMed_1 (SomeMed): 00:00
    SomeFood_1 (SomeFood): 01:00
    ...
  ```
This schedule is sorted by ascending start time.

---

## How It Works

1. **Parse the Table**  
   Each entity (like “Medication A” or “Meal B”) is read with lines such as “≥Xh apart,” “≥Yh before category,” “≥Zh after category,” etc.

2. **Clock Variables**  
   Each instance per day (e.g., 3× daily => 3 clock variables) is an integer in `[0..1440]` minutes.

3. **Building Constraints**  
   - **“Apart”** => consecutive instances are at least X hours apart.  
   - **“Before & After”** => if both appear for the same pair, unify them into a single “≥X before OR ≥Y after” big‐M disjunction.  
   - **“ApartFrom”** => similarly uses big‐M to require at least X hours in either direction.

4. **Objective**  
   - “Earliest” => sum of all times is minimized, pushing events near 0:00 if unconstrained.  
   - “Latest” => sum of all times is maximized, bunching events near 24:00 if possible.

5. **Solving**  
   The code uses [good_lp](https://docs.rs/good_lp) with a default MILP solver (CBC). If constraints are contradictory, it prints “Infeasible.”

---

## Adapting or Extending

- **Add Time Windows**: Restrict mealtimes to 7 am–10 pm by forcing clock variables within `[420..1320]`.  
- **Overnight Gaps**: If needed, add a daily constraint so the last meal is some hours before the next day’s first meal.  
- **Heavier Objectives**: Instead of just earliest or latest, you can add separate penalty terms or incorporate other scheduling goals.

---

## Troubleshooting

- **Infeasible**: Means constraints logically contradict. Check the debug lines to see which sets of constraints might be clashing.  
- **Bunching**: If you see multiple meds at 0:00 or 24:00 under “Earliest” or “Latest,” that’s normal unless you add constraints to spread them out further.

---

## License

This example is offered under MIT license. It demonstrates how to unify “≥Xh before / after” constraints with big‐M disjunctions so they don’t become contradictory. Feel free to modify or integrate it into your own scheduling needs!
