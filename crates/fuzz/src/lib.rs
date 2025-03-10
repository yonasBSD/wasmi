pub mod config;
mod crash_inputs;
mod error;
mod module;
#[cfg(feature = "differential")]
pub mod oracle;
mod value;

pub use self::{
    config::{FuzzSmithConfig, FuzzWasmiConfig},
    crash_inputs::generate_crash_inputs,
    error::{FuzzError, TrapCode},
    module::{FuzzModule, WasmSource, WatSource},
    value::{FuzzVal, FuzzValType},
};
