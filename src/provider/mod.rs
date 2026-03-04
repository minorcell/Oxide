//! Compatibility re-exports.
//!
//! New model adapters live under `crate::model_adapters`.

pub use crate::model_adapters::{ModelAdapter as ProviderAdapter, SharedAdapter};

pub mod anthropic {
    pub use crate::model_adapters::anthropic::*;
}

pub mod google {
    pub use crate::model_adapters::google::*;
}

pub mod openai {
    pub use crate::model_adapters::openai::*;
}

pub mod openai_compatible {
    pub use crate::model_adapters::openai_compatible::*;
}
