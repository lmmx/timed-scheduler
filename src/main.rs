use clock_zones::{Bound, Clock, Constraint, Variable, Zone, ZoneI64};
use std::convert::TryFrom;

// Represent the states of our automaton as events in the schedule
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Initial, // Starting state
    A,       // Event A occurred
    B,       // Event B occurred
    C,       // Event C occurred
    Final,   // All events completed
}

// Represent the transitions between events
struct Transition {
    from: Event,
    to: Event,
    guard: Option<Constraint<i64>>, // Time constraint for this transition
    reset_clock: bool,              // Whether to reset the clock after this transition
}

struct TimedAutomaton {
    states: Vec<Event>,
    transitions: Vec<Transition>,
    current_state: Event,
    zone: ZoneI64, // Current clock zone (timing constraints)
}

impl TimedAutomaton {
    fn new() -> Self {
        // Create a zone with a single clock to track time
        let zone = ZoneI64::new_zero(1);

        TimedAutomaton {
            states: vec![Event::Initial, Event::A, Event::B, Event::C, Event::Final],
            transitions: Vec::new(),
            current_state: Event::Initial,
            zone,
        }
    }

    // Check if a state is valid in this automaton
    fn is_valid_state(&self, event: Event) -> bool {
        self.states.contains(&event)
    }

    // Add a transition with timing constraint
    fn add_transition(
        &mut self,
        from: Event,
        to: Event,
        constraint: Option<Constraint<i64>>,
        reset_clock: bool,
    ) {
        // Validate that both states exist in our automaton
        if !self.is_valid_state(from) || !self.is_valid_state(to) {
            panic!("Cannot add transition between invalid states");
        }

        self.transitions.push(Transition {
            from,
            to,
            guard: constraint,
            reset_clock,
        });
    }

    // Check if a specific transition's guard is satisfied
    fn is_guard_satisfied(&self, transition_idx: usize) -> bool {
        let transition = &self.transitions[transition_idx];

        if let Some(ref guard) = transition.guard {
            // We need to recreate the constraint since we can't clone it
            let left = guard.left();
            let right = guard.right();
            let bound = guard.bound();

            // Create a new constraint with the same properties
            let new_constraint = if bound.is_strict() {
                if let Some(constant) = bound.constant() {
                    Constraint::new(left, right, i64::new_lt(constant))
                } else {
                    // Handle unbounded case
                    return true;
                }
            } else {
                if let Some(constant) = bound.constant() {
                    Constraint::new(left, right, i64::new_le(constant))
                } else {
                    // Handle unbounded case
                    return true;
                }
            };

            return self.zone.is_satisfied(new_constraint);
        }

        // If there's no guard, it's always satisfied
        true
    }

    // Try to execute a transition to the target event
    fn execute_transition(&mut self, target_event: Event) -> Result<(), &'static str> {
        // Verify the target state is valid
        if !self.is_valid_state(target_event) {
            return Err("Target event is not a valid state in this automaton");
        }

        // Find applicable transitions
        let mut succeeded = false;

        // First collect indices of transitions that match our criteria
        let applicable_transitions: Vec<usize> = self
            .transitions
            .iter()
            .enumerate()
            .filter(|(_, t)| t.from == self.current_state && t.to == target_event)
            .map(|(i, _)| i)
            .collect();

        if applicable_transitions.is_empty() {
            return Err("No transition exists from current state to target event");
        }

        // Try each transition until one succeeds
        for transition_idx in applicable_transitions {
            if self.is_guard_satisfied(transition_idx) {
                let transition = &self.transitions[transition_idx];

                // Execute the transition
                self.current_state = transition.to;

                // Reset clock if needed
                if transition.reset_clock {
                    let clock = Variable::try_from(Clock::variable(0)).unwrap();
                    self.zone.reset(clock, 0);
                }

                // Allow time to pass (future operator removes upper bounds)
                self.zone.future();

                succeeded = true;
                break;
            }
        }

        if succeeded {
            Ok(())
        } else {
            Err("No transition guard was satisfied")
        }
    }

    // Get the current timing information with emojis
    fn get_timing_info(&self) -> String {
        let clock = Clock::variable(0);
        let lower_bound = self.zone.get_lower_bound(clock);
        let upper_bound = self.zone.get_upper_bound(clock);

        // Use emojis for each state
        let state_emoji = match self.current_state {
            Event::Initial => "🚦",
            Event::A => "🅰️",
            Event::B => "🅱️",
            Event::C => "©️",
            Event::Final => "🏁",
        };

        // Format lower bound (remove Some wrapper)
        let lower_display = match lower_bound {
            Some(val) => val.to_string(),
            None => "?".to_string(),
        };

        // Format upper bound (remove Some wrapper and set ∞ without quotes)
        let upper_display = match upper_bound {
            Some(val) => val.to_string(),
            None => "∞".to_string(),
        };

        // Format the output with emojis
        format!(
            "{} : {:?}, ⏱️ : [{}, {}]",
            state_emoji, self.current_state, lower_display, upper_display
        )
    }

    // Add this method to the TimedAutomaton implementation
    fn advance_time(&mut self, time_units: i64) {
        // This simulates the passage of time by adding constraints to the zone
        let clock = Clock::variable(0);

        // Create a constraint that sets the lower bound of the clock
        // We're saying that clock >= current_min + time_units
        if let Some(current_min) = self.zone.get_lower_bound(clock) {
            let new_lower_bound = current_min + time_units;
            let constraint = Constraint::new_ge(clock, new_lower_bound);
            self.zone.add_constraint(constraint);
        }
    }
}

fn main() {
    // Create our timed automaton for scheduling
    let mut scheduler = TimedAutomaton::new();

    // Clock variable that will track time between events
    let clock = Clock::variable(0);

    // Add transitions with constraints

    // Transition from Initial to A (no time constraint)
    scheduler.add_transition(Event::Initial, Event::A, None, true);

    // Transition from A to B (B must be at least 10 time units after A)
    // Create constraint: clock >= 10
    let a_to_b_constraint = Constraint::new_ge(clock, 10);
    scheduler.add_transition(Event::A, Event::B, Some(a_to_b_constraint), true);

    // Transition from B to C (C must be between 5 and 15 time units after B)
    // We'll add constraint clock >= 5
    let b_to_c_min_constraint = Constraint::new_ge(clock, 5);
    scheduler.add_transition(Event::B, Event::C, Some(b_to_c_min_constraint), false);

    // Add upper bound constraint for B to C (C must be at most 15 time units after B)
    let b_to_c_max_constraint = Constraint::new_le(clock, 15);
    scheduler.add_transition(Event::B, Event::C, Some(b_to_c_max_constraint), false);

    // Transition from C to Final (no additional constraints)
    scheduler.add_transition(Event::C, Event::Final, None, false);

    // Simulate execution of the schedule
    println!("Starting scheduling automation");
    println!("Executed {}", scheduler.get_timing_info());

    // Execute A
    if let Err(e) = scheduler.execute_transition(Event::A) {
        println!("Error executing transition to A: {}", e);
        return;
    }
    println!("Executed {}", scheduler.get_timing_info());

    // Try to execute B immediately (should fail due to time constraint)
    if let Err(e) = scheduler.execute_transition(Event::B) {
        println!("... Failed to execute B immediately: {}", e);
    } else {
        println!("Executed B immediately (unexpected)");
    }

    // Simulate passage of time (in a real program, you would use the actual elapsed time)
    // For our example, let's pretend 15 time units have passed
    println!("⏳ Waiting 15 time units...");
    scheduler.advance_time(15);

    // Now try to execute B (should succeed)
    if let Err(e) = scheduler.execute_transition(Event::B) {
        println!("Error executing transition to B: {}", e);
        return;
    }
    println!("Executed {}", scheduler.get_timing_info());

    // Try to execute C immediately (should fail due to time constraint)
    if let Err(e) = scheduler.execute_transition(Event::C) {
        println!("... Failed to execute C immediately: {}", e);
    } else {
        println!("Executed C immediately (unexpected)");
    }

    // Simulate passage of time (in a real program, you would use the actual elapsed time)
    // For our example, let's pretend 15 time units have passed
    println!("⏳ Waiting 5 time units...");
    scheduler.advance_time(5);

    // Now try to execute C (should succeed)
    if let Err(e) = scheduler.execute_transition(Event::C) {
        println!("Error executing transition to C: {}", e);
        return;
    }
    println!("Executed {}", scheduler.get_timing_info());

    // Finally, transition to the final state
    if let Err(e) = scheduler.execute_transition(Event::Final) {
        println!("Error executing transition to Final: {}", e);
        return;
    }
    println!("Executed {}", scheduler.get_timing_info());
    println!("Schedule completed successfully");
}
