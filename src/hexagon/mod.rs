// The hexagon is vacuum-sealed: only ports (contracts) and usecases (behavior)
// are exposed. Entities are private implementation details. Any data structures
// needed by external code must be defined as plain data in ports.
// No re-exports or tricks to bypass this boundary.
mod entities;
pub mod ports;
pub mod usecases;
