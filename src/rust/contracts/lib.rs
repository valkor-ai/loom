pub mod api_quality;
pub mod architecture;
pub mod brainstorm;
pub mod code_quality;
pub mod deploy;
pub mod execution;
pub mod planning;
pub mod review;
pub mod ui_quality;

pub use api_quality::*;
pub use architecture::*;
pub use brainstorm::*;
pub use code_quality::*;
pub use deploy::*;
pub use execution::*;
pub use planning::*;
pub use review::*;
pub use ui_quality::*;

pub fn module_name() -> &'static str {
    "contracts"
}
