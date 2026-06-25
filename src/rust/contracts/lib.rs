pub mod architecture;
pub mod brainstorm;
pub mod execution;
pub mod planning;
pub mod review;

pub use architecture::*;
pub use brainstorm::*;
pub use execution::*;
pub use planning::*;
pub use review::*;

pub fn module_name() -> &'static str {
    "contracts"
}
