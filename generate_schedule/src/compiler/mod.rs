// Compiler module exports
pub mod clock_info;
pub mod constraints;
pub mod debugging;
pub mod reference_resolution;
pub mod schedule_extraction;
pub mod time_constraint_compiler;

// Re-export the primary struct
pub use time_constraint_compiler::TimeConstraintCompiler;
