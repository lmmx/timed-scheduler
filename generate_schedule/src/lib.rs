use clock_zones::{Clock, Constraint, Dbm, Variable, Zone};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// DSL Representation Structures

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Frequency {
    Daily,               // Once daily (aliases: "1x daily", "1x /d", "1x /1d")
    TwiceDaily,          // Twice daily (aliases: "2x daily", "2x /d", "2x /1d")
    ThreeTimesDaily,     // Three times daily (aliases: "3x daily", "3x /d", "3x /1d")
    EveryXHours(u8),     // Every X hours
    Custom(Vec<String>), // For custom time specifications
}

impl Frequency {
    pub fn from_str(freq_str: &str) -> Result<Self, String> {
        // Normalize the string (lowercase, remove extra spaces)
        let freq_str = freq_str.trim().to_lowercase();

        // Regular expressions for matching different formats
        let daily_re = Regex::new(r"^(daily|1x\s*daily|1x\s*/d|1x\s*/1d)$").unwrap();
        let twice_re = Regex::new(r"^(twice\s*daily|2x\s*daily|2x\s*/d|2x\s*/1d)$").unwrap();
        let thrice_re = Regex::new(r"^(thrice\s*daily|3x\s*daily|3x\s*/d|3x\s*/1d)$").unwrap();
        let every_hours_re = Regex::new(r"^every\s*(\d+)\s*hours?$").unwrap();

        if daily_re.is_match(&freq_str) {
            Ok(Frequency::Daily)
        } else if twice_re.is_match(&freq_str) {
            Ok(Frequency::TwiceDaily)
        } else if thrice_re.is_match(&freq_str) {
            Ok(Frequency::ThreeTimesDaily)
        } else if let Some(caps) = every_hours_re.captures(&freq_str) {
            let hours: u8 = caps[1]
                .parse()
                .map_err(|_| "Invalid hour format".to_string())?;
            Ok(Frequency::EveryXHours(hours))
        } else {
            Err(format!("Unrecognized frequency format: {}", freq_str))
        }
    }

    pub fn get_instances_per_day(&self) -> usize {
        match self {
            Frequency::Daily => 1,
            Frequency::TwiceDaily => 2,
            Frequency::ThreeTimesDaily => 3,
            Frequency::EveryXHours(hours) => 24 / *hours as usize,
            Frequency::Custom(times) => times.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub name: String,
    pub category: String,
    pub unit: String,
    pub amount: Option<f64>,
    pub split: Option<i32>,
    pub frequency: Frequency,
    pub constraints: Vec<ConstraintExpression>,
    pub note: Option<String>,
}

impl Entity {
    pub fn new(
        name: &str,
        category: &str,
        unit: &str,
        amount: Option<f64>,
        split: Option<i32>,
        frequency_str: &str,
        constraints: Vec<&str>,
        note: Option<&str>,
    ) -> Result<Self, String> {
        let frequency = Frequency::from_str(frequency_str)?;

        let constraint_expressions = constraints
            .into_iter()
            .map(|s| ConstraintExpression::parse(s))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Entity {
            name: name.to_string(),
            category: category.to_string(),
            unit: unit.to_string(),
            amount,
            split,
            frequency,
            constraints: constraint_expressions,
            note: note.map(|s| s.to_string()),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TimeUnit {
    Minute,
    Hour,
}

impl TimeUnit {
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_lowercase().as_str() {
            "m" | "min" | "minute" | "minutes" => Ok(TimeUnit::Minute),
            "h" | "hr" | "hour" | "hours" => Ok(TimeUnit::Hour),
            _ => Err(format!("Unknown time unit: {}", s)),
        }
    }

    fn to_minutes(&self, value: u32) -> u32 {
        match self {
            TimeUnit::Minute => value,
            TimeUnit::Hour => value * 60,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintType {
    Before,    // Target must be scheduled before reference
    After,     // Target must be scheduled after reference
    ApartFrom, // Target must be separated from reference (both before and after)
    Apart,     // Used within recurring instances of the same entity
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintExpression {
    pub time_value: u32,
    pub time_unit: TimeUnit,
    pub constraint_type: ConstraintType,
    pub reference: ConstraintReference,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConstraintReference {
    Entity(String),   // Reference to a specific entity by name
    Category(String), // Reference to all entities in a category
    WithinGroup,      // For 'apart' constraints within recurring instances
}

impl ConstraintExpression {
    pub fn parse(expr: &str) -> Result<Self, String> {
        // Clean up the input string
        let expr = expr.trim();

        // Regular expressions for different constraint patterns
        let before_re = Regex::new(r"^≥(\d+)([hm])\s+before\s+(.+)$").unwrap();
        let after_re = Regex::new(r"^≥(\d+)([hm])\s+after\s+(.+)$").unwrap();
        let apart_from_re = Regex::new(r"^≥(\d+)([hm])\s+apart\s+from\s+(.+)$").unwrap();
        let apart_re = Regex::new(r"^≥(\d+)([hm])\s+apart$").unwrap();

        if let Some(caps) = before_re.captures(expr) {
            let time_value: u32 = caps[1]
                .parse()
                .map_err(|_| "Invalid time value".to_string())?;
            let time_unit = TimeUnit::from_str(&caps[2])?;
            let reference = parse_reference(&caps[3])?;

            Ok(ConstraintExpression {
                time_value,
                time_unit,
                constraint_type: ConstraintType::Before,
                reference,
            })
        } else if let Some(caps) = after_re.captures(expr) {
            let time_value: u32 = caps[1]
                .parse()
                .map_err(|_| "Invalid time value".to_string())?;
            let time_unit = TimeUnit::from_str(&caps[2])?;
            let reference = parse_reference(&caps[3])?;

            Ok(ConstraintExpression {
                time_value,
                time_unit,
                constraint_type: ConstraintType::After,
                reference,
            })
        } else if let Some(caps) = apart_from_re.captures(expr) {
            let time_value: u32 = caps[1]
                .parse()
                .map_err(|_| "Invalid time value".to_string())?;
            let time_unit = TimeUnit::from_str(&caps[2])?;
            let reference = parse_reference(&caps[3])?;

            Ok(ConstraintExpression {
                time_value,
                time_unit,
                constraint_type: ConstraintType::ApartFrom,
                reference,
            })
        } else if let Some(caps) = apart_re.captures(expr) {
            let time_value: u32 = caps[1]
                .parse()
                .map_err(|_| "Invalid time value".to_string())?;
            let time_unit = TimeUnit::from_str(&caps[2])?;

            Ok(ConstraintExpression {
                time_value,
                time_unit,
                constraint_type: ConstraintType::Apart,
                reference: ConstraintReference::WithinGroup,
            })
        } else {
            Err(format!("Could not parse constraint expression: {}", expr))
        }
    }
}

fn parse_reference(reference: &str) -> Result<ConstraintReference, String> {
    let reference = reference.trim().to_lowercase();

    if reference.starts_with("category:") {
        let category = reference.trim_start_matches("category:").trim();
        Ok(ConstraintReference::Category(category.to_string()))
    } else {
        // Default to an entity reference
        Ok(ConstraintReference::Entity(reference.to_string()))
    }
}

// Compiler from DSL to clock-zones

pub struct ClockInfo {
    pub entity_name: String,
    pub instance: usize,
    pub variable: Variable,
}

pub struct TimeConstraintCompiler {
    // Maps entity names to their data
    entities: HashMap<String, Entity>,
    // Maps category names to sets of entity names
    categories: HashMap<String, HashSet<String>>,
    // Maps clock IDs to their information
    clocks: HashMap<String, ClockInfo>,
    // The generated zone with constraints
    zone: Dbm<i64>,
    // Next available clock variable index
    next_clock_index: usize,
}

impl TimeConstraintCompiler {
    pub fn new(entities: Vec<Entity>) -> Self {
        // Organize entities and categories
        let mut entity_map = HashMap::new();
        let mut category_map: HashMap<String, HashSet<String>> = HashMap::new();

        for entity in entities {
            // Add to category map
            category_map
                .entry(entity.category.clone())
                .or_default()
                .insert(entity.name.clone());

            // Add to entity map
            entity_map.insert(entity.name.clone(), entity);
        }

        // Calculate total clock variables needed
        let total_clocks = entity_map
            .values()
            .map(|e| e.frequency.get_instances_per_day())
            .sum();

        let zone = Dbm::new_zero(total_clocks);

        TimeConstraintCompiler {
            entities: entity_map,
            categories: category_map,
            clocks: HashMap::new(),
            zone,
            next_clock_index: 0,
        }
    }

    pub fn compile(&mut self) -> Result<&Dbm<i64>, String> {
        // 1. Create clock variables for all entity instances
        self.allocate_clocks()?;

        // 2. Set daily bounds (0-24 hours in minutes)
        self.set_daily_bounds()?;

        // 3. Apply frequency-based constraints (spacing between occurrences)
        self.apply_frequency_constraints()?;

        // 4. Apply entity-specific constraints
        self.apply_entity_constraints()?;

        // 5. Check feasibility
        if self.zone.is_empty() {
            return Err("Schedule is not feasible with the given constraints".to_string());
        }

        Ok(&self.zone)
    }

    fn allocate_clocks(&mut self) -> Result<(), String> {
        for (entity_name, entity) in &self.entities {
            let instances = entity.frequency.get_instances_per_day();

            for i in 0..instances {
                let clock_id = format!("{}_{}", entity_name, i + 1);
                let variable = Clock::variable(self.next_clock_index);
                self.next_clock_index += 1;

                self.clocks.insert(
                    clock_id.clone(),
                    ClockInfo {
                        entity_name: entity_name.clone(),
                        instance: i + 1,
                        variable,
                    },
                );
            }
        }

        Ok(())
    }

    fn set_daily_bounds(&mut self) -> Result<(), String> {
        // Convert time to minutes (0-1440 for a 24-hour day)
        for clock_info in self.clocks.values() {
            // Not before 0:00
            self.zone
                .add_constraint(Constraint::new_ge(clock_info.variable, 0));
            // Not after 23:59
            self.zone
                .add_constraint(Constraint::new_le(clock_info.variable, 1439));
        }

        Ok(())
    }

    fn apply_frequency_constraints(&mut self) -> Result<(), String> {
        // Group clocks by entity
        let mut entity_clocks: HashMap<String, Vec<Variable>> = HashMap::new();

        for clock_info in self.clocks.values() {
            entity_clocks
                .entry(clock_info.entity_name.clone())
                .or_default()
                .push(clock_info.variable);
        }

        // For each entity, ensure instance ordering and apply default spacing
        for (entity_name, clocks) in entity_clocks {
            if clocks.len() <= 1 {
                continue; // No constraints needed for single instances
            }

            let entity = self.entities.get(&entity_name).unwrap();

            // Sort clocks by instance number
            let mut ordered_clocks: Vec<(usize, Variable)> = self
                .clocks
                .values()
                .filter(|c| c.entity_name == entity_name)
                .map(|c| (c.instance, c.variable))
                .collect();
            ordered_clocks.sort_by_key(|&(instance, _)| instance);

            // Apply ordering and spacing constraints
            for i in 0..ordered_clocks.len() - 1 {
                let (_, current) = ordered_clocks[i];
                let (_, next) = ordered_clocks[i + 1];

                // Next instance must come after current instance
                self.zone
                    .add_constraint(Constraint::new_diff_gt(next, current, 0));

                // Apply minimum spacing based on frequency
                let min_spacing = match entity.frequency {
                    Frequency::TwiceDaily => 6 * 60,      // 6 hours in minutes
                    Frequency::ThreeTimesDaily => 4 * 60, // 4 hours in minutes
                    Frequency::EveryXHours(hours) => (hours as i64) * 60,
                    _ => 60, // Default 1 hour minimum spacing
                };

                self.zone
                    .add_constraint(Constraint::new_diff_ge(next, current, min_spacing));
            }
        }

        Ok(())
    }

    fn apply_entity_constraints(&mut self) -> Result<(), String> {
        // Collect all constraints first to avoid borrowing issues
        let mut all_constraints: Vec<(String, ConstraintExpression)> = Vec::new();

        for (entity_name, entity) in &self.entities {
            for constraint in &entity.constraints {
                all_constraints.push((entity_name.clone(), constraint.clone()));
            }
        }

        // Now apply all collected constraints
        for (entity_name, constraint) in all_constraints {
            self.apply_constraint(&entity_name, &constraint)?;
        }

        Ok(())
    }

    fn apply_constraint(
        &mut self,
        entity_name: &str,
        constraint: &ConstraintExpression,
    ) -> Result<(), String> {
        // Convert time value to minutes
        let time_in_minutes = constraint.time_unit.to_minutes(constraint.time_value) as i64;

        // Get all clocks for this entity
        let entity_clocks: Vec<Variable> = self
            .clocks
            .values()
            .filter(|c| c.entity_name == entity_name)
            .map(|c| c.variable)
            .collect();

        match &constraint.constraint_type {
            ConstraintType::Apart => {
                // Apply spacing constraint between instances of the same entity
                if entity_clocks.len() <= 1 {
                    // No constraints needed for single instance
                    return Ok(());
                }

                for i in 0..entity_clocks.len() {
                    for j in i + 1..entity_clocks.len() {
                        // Ensure minimum spacing in either direction
                        self.zone.add_constraint(Constraint::new_diff_ge(
                            entity_clocks[i],
                            entity_clocks[j],
                            time_in_minutes,
                        ));
                        self.zone.add_constraint(Constraint::new_diff_ge(
                            entity_clocks[j],
                            entity_clocks[i],
                            time_in_minutes,
                        ));
                    }
                }
            }

            ConstraintType::Before | ConstraintType::After | ConstraintType::ApartFrom => {
                // Get reference clocks based on the constraint reference
                let reference_clocks = self.get_reference_clocks(&constraint.reference)?;

                for &entity_clock in &entity_clocks {
                    for &reference_clock in &reference_clocks {
                        match constraint.constraint_type {
                            ConstraintType::Before => {
                                // Entity must be scheduled at least X minutes before reference
                                self.zone.add_constraint(Constraint::new_diff_ge(
                                    reference_clock,
                                    entity_clock,
                                    time_in_minutes,
                                ));
                            }
                            ConstraintType::After => {
                                // Entity must be scheduled at least X minutes after reference
                                self.zone.add_constraint(Constraint::new_diff_ge(
                                    entity_clock,
                                    reference_clock,
                                    time_in_minutes,
                                ));
                            }
                            ConstraintType::ApartFrom => {
                                // Entity must be separated from reference by at least X minutes
                                // in either direction
                                self.zone.add_constraint(Constraint::new_diff_ge(
                                    entity_clock,
                                    reference_clock,
                                    time_in_minutes,
                                ));
                                self.zone.add_constraint(Constraint::new_diff_ge(
                                    reference_clock,
                                    entity_clock,
                                    time_in_minutes,
                                ));
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn get_reference_clocks(
        &self,
        reference: &ConstraintReference,
    ) -> Result<Vec<Variable>, String> {
        match reference {
            ConstraintReference::Entity(name) => {
                // Get clocks for a specific entity
                let clocks = self
                    .clocks
                    .values()
                    .filter(|c| c.entity_name == *name)
                    .map(|c| c.variable)
                    .collect::<Vec<_>>();

                if clocks.is_empty() {
                    return Err(format!("No clocks found for entity: {}", name));
                }

                Ok(clocks)
            }

            ConstraintReference::Category(category) => {
                // Get clocks for all entities in a category
                let entities = self
                    .categories
                    .get(category)
                    .ok_or_else(|| format!("Category not found: {}", category))?;

                let clocks = self
                    .clocks
                    .values()
                    .filter(|c| entities.contains(&c.entity_name))
                    .map(|c| c.variable)
                    .collect::<Vec<_>>();

                if clocks.is_empty() {
                    return Err(format!("No clocks found for category: {}", category));
                }

                Ok(clocks)
            }

            ConstraintReference::WithinGroup => {
                // This should not be called directly - handled by the Apart constraint
                Err("WithinGroup reference should not be accessed directly".to_string())
            }
        }
    }

    // Extract a concrete schedule from the zone
    pub fn extract_schedule(&self) -> Result<HashMap<String, i32>, String> {
        if self.zone.is_empty() {
            return Err("Cannot extract schedule from empty zone".to_string());
        }

        let mut schedule = HashMap::new();

        // For each clock, get a feasible time
        for (clock_id, clock_info) in &self.clocks {
            // Get the lower and upper bounds for this clock (in minutes)
            let lower = self.zone.get_lower_bound(clock_info.variable).unwrap_or(0);
            let upper = self
                .zone
                .get_upper_bound(clock_info.variable)
                .unwrap_or(1439);

            // Choose a time in the middle of the feasible range
            let time_in_minutes = ((lower + upper) / 2) as i32;
            schedule.insert(clock_id.clone(), time_in_minutes);
        }

        Ok(schedule)
    }

    // Format the schedule into a human-readable format
    pub fn format_schedule(&self, schedule: &HashMap<String, i32>) -> String {
        let mut result = String::new();
        result.push_str("Daily Schedule:\n");

        // Convert minutes to HH:MM format and sort by time
        let mut time_entries: Vec<(String, String)> = schedule
            .iter()
            .map(|(clock_id, &minutes)| {
                let hours = minutes / 60;
                let mins = minutes % 60;
                let time_str = format!("{:02}:{:02}", hours, mins);
                (time_str.clone(), format!("{}: {}", clock_id, time_str))
            })
            .collect();

        // Sort by time
        time_entries.sort_by(|a, b| a.0.cmp(&b.0));

        // Add to result
        for (_, entry) in time_entries {
            result.push_str(&format!("  {}\n", entry));
        }

        // Group by entity
        result.push_str("\nBy Entity:\n");

        let mut entity_schedules: HashMap<String, Vec<(String, i32)>> = HashMap::new();

        for (clock_id, &minutes) in schedule {
            if let Some(clock_info) = self.clocks.get(clock_id) {
                entity_schedules
                    .entry(clock_info.entity_name.clone())
                    .or_default()
                    .push((clock_id.clone(), minutes));
            }
        }

        // Sort entities alphabetically
        let mut entity_names: Vec<String> = entity_schedules.keys().cloned().collect();
        entity_names.sort();

        for entity_name in entity_names {
            let entity = self.entities.get(&entity_name).unwrap();
            result.push_str(&format!("  {} ({}):\n", entity_name, entity.category));

            let times = entity_schedules.get(&entity_name).unwrap();
            let mut sorted_times = times.clone();
            sorted_times.sort_by_key(|&(_, minutes)| minutes);

            for (clock_id, minutes) in sorted_times {
                let hours = minutes / 60;
                let mins = minutes % 60;
                result.push_str(&format!("    {}: {:02}:{:02}", clock_id, hours, mins));

                // Add amount information if available
                if let Some(entity) = self.entities.get(&entity_name) {
                    if let Some(amount) = entity.amount {
                        if let Some(split) = entity.split {
                            // If we have both amount and split
                            let per_instance = amount / split as f64;
                            result.push_str(&format!(" - {:.1} {}", per_instance, entity.unit));
                        } else {
                            // If we have just amount
                            result.push_str(&format!(" - {:.1} {}", amount, entity.unit));
                        }
                    } else if let Some(split) = entity.split {
                        // If we have just split
                        result.push_str(&format!(" - 1/{} {}", split, entity.unit));
                    }
                }

                result.push_str("\n");
            }
        }

        result
    }
}

// Function to parse from the tabular format shown in the example
pub fn parse_from_table(rows: Vec<Vec<&str>>) -> Result<Vec<Entity>, String> {
    let mut entities = Vec::new();

    // Skip header row
    for row in rows.iter().skip(1) {
        if row.len() < 7 {
            return Err("Row has insufficient columns".to_string());
        }

        let name = row[0];
        let category = row[1];
        let unit = row[2];

        // Parse amount (float or null)
        let amount = match row[3] {
            "null" => None,
            s => Some(
                s.parse::<f64>()
                    .map_err(|_| "Invalid amount format".to_string())?,
            ),
        };

        // Parse split (int or null)
        let split = match row[4] {
            "null" => None,
            s => Some(
                s.parse::<i32>()
                    .map_err(|_| "Invalid split format".to_string())?,
            ),
        };

        let frequency = row[5];

        // Parse constraints array (from string to vec)
        let constraints_str = row[6].trim();
        let constraints = if constraints_str == "[]" {
            Vec::new()
        } else {
            // Extract strings between quotes inside the array
            let re = Regex::new(r#""([^"]+)""#).unwrap();
            re.captures_iter(constraints_str)
                .map(|cap| cap[1].to_string())
                .collect::<Vec<String>>()
        };

        let note = match row[7] {
            "null" => None,
            s => Some(s),
        };

        entities.push(Entity::new(
            name,
            category,
            unit,
            amount,
            split,
            frequency,
            constraints.iter().map(|s| s.as_str()).collect(),
            note,
        )?);
    }

    Ok(entities)
}

// Example of usage with the provided table data
fn example() -> Result<(), String> {
    // This would come from parsing the table
    let table_data = vec![
        vec![
            "Entity",
            "Category",
            "Unit",
            "Amount",
            "Split",
            "Frequency",
            "Constraints",
            "Note",
        ],
        vec![
            "Antepsin",
            "med",
            "tablet",
            "null",
            "3",
            "3x daily",
            "[\"≥1h before food\", \"≥2h after food\", \"≥2h apart from med\", \"≥6h apart\"]",
            "in 1tsp water",
        ],
        vec![
            "Gabapentin",
            "med",
            "ml",
            "1.8",
            "null",
            "2x daily",
            "[\"≥8h apart\", \"≥30m before food or med\"]",
            "null",
        ],
        vec![
            "Pardale",
            "med",
            "tablet",
            "null",
            "2",
            "2x daily",
            "[\"≥30m before food or med\", \"≥8h apart\"]",
            "null",
        ],
        vec![
            "Pro-Kolin",
            "med",
            "ml",
            "3.0",
            "null",
            "2x daily",
            "[]",
            "with food",
        ],
        vec![
            "Omeprazole",
            "med",
            "capsule",
            "null",
            "null",
            "1x daily",
            "[\"≥30m before food\"]",
            "null",
        ],
        vec![
            "Chicken and rice",
            "food",
            "meal",
            "null",
            "null",
            "2x daily",
            "[]",
            "null",
        ],
    ];

    let entities = parse_from_table(table_data)?;

    // Create compiler and generate schedule
    let mut compiler = TimeConstraintCompiler::new(entities);
    let zone = compiler.compile()?;

    // Check if feasible
    if zone.is_empty() {
        println!("Schedule is not feasible");
        return Err("Schedule is not feasible".to_string());
    }

    // Extract a concrete schedule
    let schedule = compiler.extract_schedule()?;

    // Display formatted schedule
    let formatted = compiler.format_schedule(&schedule);
    println!("{}", formatted);

    Ok(())
}
