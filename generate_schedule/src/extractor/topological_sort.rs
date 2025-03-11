use std::collections::{HashMap, VecDeque};
use crate::compiler::clock_info::ClockInfo;
use clock_zones::{Bound, Zone};
use colored::Colorize;
use crate::extractor::schedule_extractor::ScheduleExtractor;

impl<'a> ScheduleExtractor<'a> {
    // Build a dependency graph from constraints in the zone
    pub fn build_dependency_graph(&self) -> (
        HashMap<String, Vec<String>>,
        HashMap<String, usize>
    ) {
        self.debug_print("🔗", "Building dependency graph from constraints");

        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Initialize maps
        for (clock_id, _) in self.clocks.iter() {
            adjacency.insert(clock_id.clone(), Vec::new());
            in_degree.insert(clock_id.clone(), 0);
        }

        // Group clocks by entity
        let mut entity_clocks: HashMap<String, Vec<(String, usize)>> = HashMap::new();
        for (clock_id, info) in self.clocks.iter() {
            entity_clocks
                .entry(info.entity_name.clone())
                .or_insert_with(Vec::new)
                .push((clock_id.clone(), info.instance));
        }

        // Add edges for instance ordering within each entity
        for (_, clocks) in entity_clocks.iter() {
            let mut sorted_clocks = clocks.clone();
            sorted_clocks.sort_by_key(|&(_, instance)| instance);

            for i in 0..sorted_clocks.len() - 1 {
                let (from_id, _) = &sorted_clocks[i];
                let (to_id, _) = &sorted_clocks[i + 1];

                // Instance i must come before instance i+1
                adjacency.get_mut(from_id).unwrap().push(to_id.clone());
                *in_degree.get_mut(to_id).unwrap() += 1;

                if self.debug {
                    self.debug_print("➡️", &format!(
                        "Added edge: {} must come before {}",
                        from_id, to_id
                    ));
                }
            }
        }

        // Add edges from difference constraints in the DBM
        for (id_i, info_i) in self.clocks.iter() {
            for (id_j, info_j) in self.clocks.iter() {
                if id_i == id_j {
                    continue;
                }

                // Check if there is a constraint: j must be at least min_diff after i
                if let Some(bound) = self.zone.get_bound(info_i.variable, info_j.variable).constant() {
                    let min_diff = -bound;
                    if min_diff > 0 {
                        // There is a non-trivial difference constraint
                        adjacency.get_mut(id_i).unwrap().push(id_j.clone());
                        *in_degree.get_mut(id_j).unwrap() += 1;

                        if self.debug {
                            self.debug_print("🔗", &format!(
                                "Added edge from constraint: {} must be ≥{}m after {}",
                                id_j, min_diff, id_i
                            ));
                        }
                    }
                }
            }
        }

        (adjacency, in_degree)
    }

    // Improved topological sort with interleaving heuristic
    pub fn sort_clocks_topologically(&self) -> Vec<(String, &ClockInfo)> {
        self.debug_print("🔄", "Sorting clocks with interleaving topological order");

        // Build the dependency graph
        let (adjacency, mut in_degree) = self.build_dependency_graph();

        // Initialize ready queue with nodes that have no dependencies
        let mut ready_queue: VecDeque<String> = VecDeque::new();
        for (clock_id, &degree) in in_degree.iter() {
            if degree == 0 {
                ready_queue.push_back(clock_id.clone());
                if self.debug {
                    self.debug_print("🔄", &format!("Added {} to initial ready queue", clock_id));
                }
            }
        }

        // Result will be in topological order
        let mut sorted_clocks: Vec<(String, &ClockInfo)> = Vec::new();

        // Track the last entity picked to help with interleaving
        let mut last_entity_picked: Option<String> = None;

        // Process the ready queue, prioritizing different entities
        while !ready_queue.is_empty() {
            // Try to find a node from a different entity than the last one
            let node_index = self.pick_next_node(&ready_queue, &last_entity_picked);

            // Remove the chosen node from the ready queue
            let node_id = if node_index == 0 {
                ready_queue.pop_front().unwrap()
            } else {
                // Remove from arbitrary position
                let node_id = ready_queue[node_index].clone();
                ready_queue.remove(node_index);
                node_id
            };

            // Add to our sorted result
            let node_info = self.clocks.get(&node_id).unwrap();
            sorted_clocks.push((node_id.clone(), node_info));

            // Update the last entity picked
            last_entity_picked = Some(node_info.entity_name.clone());

            // Update dependencies and potentially add new nodes to ready queue
            for successor in adjacency.get(&node_id).unwrap() {
                let new_degree = in_degree.get_mut(successor).unwrap().saturating_sub(1);
                *in_degree.get_mut(successor).unwrap() = new_degree;

                if new_degree == 0 {
                    ready_queue.push_back(successor.clone());
                    if self.debug {
                        self.debug_print("🔄", &format!(
                            "Added {} to ready queue after processing {}",
                            successor, node_id
                        ));
                    }
                }
            }
        }

        // Verify we've processed all nodes (sanity check for cycles)
        if sorted_clocks.len() != self.clocks.len() {
            self.debug_error("⚠️", &format!(
                "Topological sort only found {} nodes out of {}. There may be cycles in constraints.",
                sorted_clocks.len(), self.clocks.len()
            ));
        }

        if self.debug {
            self.debug_print("📋", "Final topologically sorted clock order:");
            for (i, (id, info)) in sorted_clocks.iter().enumerate() {
                println!("   {}. {} ({}, instance {})",
                         i+1, id.cyan(), info.entity_name.blue(), info.instance);

                // Also show bounds for debugging
                let bounds = self.get_bounds(info.variable);
                self.debug_bounds(id, &bounds);
            }
        }

        sorted_clocks
    }

    // Helper to find a node from a different entity than the last one, if possible
    pub fn pick_next_node(&self, ready_queue: &VecDeque<String>, last_entity: &Option<String>) -> usize {
        if let Some(last_entity_name) = last_entity {
            // Try to find an entity different from the last one
            for (index, node_id) in ready_queue.iter().enumerate() {
                let info = self.clocks.get(node_id).unwrap();
                if &info.entity_name != last_entity_name {
                    self.debug_print("🔀", &format!(
                        "Interleaving: picked {} (entity {}) to avoid repeating {}",
                        node_id, info.entity_name, last_entity_name
                    ));
                    return index;
                }
            }
        }

        // If no different entity found or no last entity, pick the first one
        0
    }
}
