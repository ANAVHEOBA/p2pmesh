// Gateway module - Settlement Bridge
// Handles collecting IOUs and settling them to external systems (banks, blockchains)

mod collector;
mod settler;

pub use collector::*;
pub use settler::*;
