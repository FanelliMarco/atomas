pub mod elements;
pub mod game;
pub mod ring;
pub mod solver;

// Re-export commonly used items
pub use elements::{Data, Element, ElementType, Id, SpecialAtom};
pub use game::{Action, Atom, GameState};
pub use solver::{Solver, SolverConfig, SolverResult, solve, solve_fast};
