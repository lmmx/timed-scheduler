# Check schedule

> Using the clock-zones crate for timed automata

Check the feasibility of a given schedule.

Run `cargo run` to show the demo

```
🔍 Analyzing medication schedule constraints...
💊 Medicine A must be taken at least 2 hours apart
🍽️  Medicine A must be taken at least 30 minutes after food

📊 Schedule Analysis Results:
✅ Constraints valid! Schedule possible.
💊 Earliest time for Medicine A: 2h (120)
🍽️  Earliest time for food: 0min (0)

⏳ After allowing time to pass:
✅ Future schedule possibilities remain feasible.
💊 Medicine A lower bound: 120
💊 Medicine A has no upper bound ∞

📝 Schedule Summary:
- Take food first 🍽️
- Wait at least 30 minutes ⏱️
- Take Medicine A 💊
- Wait at least 2 hours before next dose 🕒
```
