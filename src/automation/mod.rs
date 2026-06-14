//! Milestone 2: Automation Loop
//!
//! This module implements the automation loop for playing Atomas:
//! Capture → Detect → Decide → Execute → Repeat
//!
//! Phase 1: Dry-run mode only (no ADB required)
//! Phase 2: ADB integration

pub mod adb;
pub mod capture;
pub mod decision;
pub mod executor;
pub mod loop_controller;
mod expectimax_solver;

pub use capture::ScreenshotSource;
pub use decision::{SimpleSolver, SimpleStrategy};
pub use executor::ActionExecutor;
pub use loop_controller::AutomationLoop;
pub use expectimax_solver::{ExpectimaxSolver, SolverStrategy};
