# timed schedule

## Simulate execution

Using the clock-zones crate for timed automata: `simulate_execution`

This is the least interesting way to use symbolic representations of time constraints
but also the most obviously practical one.

Run `cargo run` to show the demo

```
Starting scheduling automation
Executed 🚦 : Initial, ⏱️ : [0, 0]
Executed 🅰️ : A, ⏱️ : [0, ∞]
... Failed to execute B immediately: No transition guard was satisfied
⏳ Waiting 15 time units...
Executed 🅱️ : B, ⏱️ : [0, ∞]
... Failed to execute C immediately: No transition guard was satisfied
⏳ Waiting 5 time units...
Executed ©️ : C, ⏱️ : [5, ∞]
Executed 🏁 : Final, ⏱️ : [5, ∞]
Schedule completed successfully
```
