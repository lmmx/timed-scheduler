use crate::compiler::time_constraint_compiler::TimeConstraintCompiler;
use clock_zones::Variable;

// Enhanced resolve_reference method to handle "or" expressions without sorting
pub fn resolve_reference(
    compiler: &TimeConstraintCompiler,
    reference_str: &str,
) -> Result<Vec<Variable>, String> {
    // Check if the reference contains " or "
    if reference_str.contains(" or ") {
        let parts: Vec<&str> = reference_str.split(" or ").collect();
        let mut all_clocks = Vec::new();

        // Resolve each part separately and combine the results
        for part in parts {
            match resolve_single_reference(compiler, part.trim()) {
                Ok(clocks) => {
                    // Add only unique clocks
                    for clock in clocks {
                        if !all_clocks.contains(&clock) {
                            all_clocks.push(clock);
                        }
                    }
                }
                Err(_) => (), // Ignore errors for individual parts in an OR expression
            }
        }

        if all_clocks.is_empty() {
            return Err(format!(
                "Could not resolve any part of reference '{}'",
                reference_str
            ));
        }

        return Ok(all_clocks);
    }

    // If no "or", just resolve as a single reference
    resolve_single_reference(compiler, reference_str)
}

// Helper method to resolve a single reference (no OR)
pub fn resolve_single_reference(
    compiler: &TimeConstraintCompiler,
    reference_str: &str,
) -> Result<Vec<Variable>, String> {
    // First try to find it as an entity (exact match)
    let entity_clocks: Vec<Variable> = compiler
        .clocks
        .values()
        .filter(|c| c.entity_name.to_lowercase() == reference_str.to_lowercase())
        .map(|c| c.variable)
        .collect();

    if !entity_clocks.is_empty() {
        return Ok(entity_clocks);
    }

    // If not found as entity, try as a category
    if let Some(entities) = compiler.categories.get(reference_str) {
        let category_clocks: Vec<Variable> = compiler
            .clocks
            .values()
            .filter(|c| entities.contains(&c.entity_name))
            .map(|c| c.variable)
            .collect();

        if !category_clocks.is_empty() {
            return Ok(category_clocks);
        }
    }

    // If still not found, return an error
    Err(format!(
        "Could not resolve reference '{}' - not found as entity or category",
        reference_str
    ))
}
