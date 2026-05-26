//! Public API for embedding Trellis.

pub mod engine;
pub mod importer;
#[cfg(feature = "install")]
pub mod installer;
pub mod parser;
pub mod pipeline;
pub mod report;
pub mod resolver;

pub use engine::{Client, ExecOpts, Execution};
pub use parser::{Endpoint, EnvConfig, Pipeline};
pub use pipeline::{run as run_pipeline, PipelineResult};
pub use resolver::{mask, Context};
