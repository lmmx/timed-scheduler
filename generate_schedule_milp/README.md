Below is a revised **README** reflecting the current (post‐rewrite) code and its major features, including “before/after” constraints **and** the new scheduling logic that can optionally spread tasks across windows.

---

# Timed Scheduler

A **mixed‐integer linear programming (MILP)** tool for scheduling events (e.g. medications and meals) under various timing rules. It merges constraints like “≥6 h apart,” “≥1 h before [some category],” or “≥2 h after [some category]” into **big‐M disjunctions** when both apply to the same pair, ensuring those conditions are treated as **OR** rather than **AND**. If constraints are contradictory, the solver finds it infeasible; otherwise, it yields a feasible schedule.

The code supports an **Earliest** or **Latest** objective, plus additional constraints or soft penalties to encourage placing items within preferred windows (e.g., mealtimes) or distributing multiple daily instances across distinct windows (e.g., breakfast vs. dinner).

---

## Key Features

1. **Constraints via Regex & Big‑M**
   - **`≥Xh apart`** (same entity’s consecutive instances).
   - **`≥Xh before SomeCategory`** or **`≥Xh after SomeCategory`** (inter‐entity offsets).
   - If _both_ “≥1 h before” and “≥2 h after” appear for the same pair, they become a single big‑M disjunction **(before OR after)**—avoiding contradictory “≥1h before AND ≥2h after” for the same referent.

2. **Earliest / Latest Objective**
   - **Earliest**: Minimizes sum of start times, pushing tasks as early in the day as possible.
   - **Latest**: Maximizes sum of start times (or equivalently, minimizes the negative sum), pushing tasks toward the end of the day window.

3. **Time Windows & Distribution**
   - The new code can define “soft windows” (anchors or ranges) and penalize deviation, ensuring tasks stay near those times.
   - Optionally, it can define **binary “window usage”** constraints so multiple daily doses get distributed across windows (e.g., “breakfast,” “lunch,” “dinner”). That can prevent tasks from bunching in a single boundary time.

4. **Debug Logging**
   - Prints lines like `(Before|After) (food_var2) - (med_var1) >= 60 - M*(1-b)` so you can see the exact constraints.
   - Provides a “Window Usage Report” or a “Penalty Report” if you’re using the distribution or soft penalty logic, showing how many tasks end up in each time slot and how far off from ideal anchors they are.

5. **Table‐Driven**
   - A small table in `main.rs` describes each entity: frequency (2× daily, 3× daily, etc.), constraints (like `[\"≥6h apart\"]`), optional windows (e.g. `[\"08:00\", \"18:00-20:00\"]`).
   - The code converts these lines into entity objects with constraints and then to ILP variables & constraints, all solved with [good_lp](https://docs.rs/good_lp) using the CBC solver by default.

---

## Usage

1. **Run**:
   ```bash
   cargo run -- --strategy earliest
   cargo run -- --strategy latest
   ```
   By default it uses a day window of 08:00–22:00. You can override with e.g. `--start=07:00 --end=23:00`.

2. **Check Debug Output**:
   - You’ll see `DEBUG => (Apart) (Entity_var2) - (Entity_var1) >= 360` lines showing each big‑M or linear constraint.
   - Finally, the solver prints a schedule sorted by minute of day, plus optional “Window usage” or “Penalty” info.

3. **Adapt / Tweak**:
   - If your domain needs “3× daily spread across breakfast, lunch, dinner,” you can define 3 windows and enforce “one instance per window,” or add a penalty for leaving windows unused.
   - If you only want “Earliest” or “Latest” with no distribution logic, remove or skip the extra window constraints.

---

## Example Output

After running with “Earliest,” you might see:

```
--- Final schedule (Earliest) ---
  Antepsin_1 (Antepsin): 07:00
  ...
  Chicken and rice_1 (Chicken and rice): 08:00
  Chicken and rice_2 (Chicken and rice): 17:30
  ...
```
```
--- Window Usage Report ---
  Entity          Window        Used By
  Chicken and rice  08:00         #1
  Chicken and rice  18:00-20:00   #2
```
```
--- Window Adherence Report ---
  Entity      | Instance   | Deviation | Preferred Windows
  Chicken and rice | #1 (08:00) | On target  | 08:00, 18:00-20:00
  Chicken and rice | #2 (17:30) | On target  | 08:00, 18:00-20:00
  Total penalty: 0.0
```

---

## Future Extensions

- **Penalize Large Gaps**: If you want tasks in windows 1 & 3 to require window 2 be used, you can add an extra penalty if windows are used out of order.
- **Multi‐Objective**: Combine earliest/largest with a second objective for minimal transitions, or ensure balanced usage.
- **Real‐Time** or **Multi‐Day** Scheduling**: Extend the day window beyond 24 h or create separate sets of variables for multiple days.

---

## License

Provided under the MIT license. This project demonstrates a flexible MILP approach to scheduling with big‐M logic for “before/after” constraints, plus optional windows and distribution. Use or adapt to your own domain as needed. Have fun scheduling!
