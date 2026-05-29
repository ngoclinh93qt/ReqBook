//! Public API for embedding MarkApiDown.

pub mod adhoc;
pub mod engine;
pub mod history;
pub mod importer;
#[cfg(feature = "install")]
pub mod installer;
pub mod mcp;
#[cfg(feature = "web")]
pub mod mock;
pub mod parser;
pub mod pipeline;
#[cfg(feature = "web")]
pub mod preview;
pub mod report;
pub mod resolver;
pub mod workspace;

pub use engine::{Client, ExecOpts, Execution};
pub use parser::{Endpoint, EnvConfig, Pipeline};
pub use pipeline::{run as run_pipeline, PipelineResult};
pub use resolver::{mask, Context};
