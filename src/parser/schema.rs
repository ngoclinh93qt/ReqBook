//! Data model types for Reqbook specs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Parsed endpoint specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Endpoint {
    /// Source path when loaded from disk.
    pub source: Option<String>,
    /// Endpoint frontmatter.
    pub schema: EndpointSchema,
    /// Sentence-case title from the H1.
    pub title: String,
    /// First paragraph after the title.
    pub description: String,
    /// Executable HTTP request block.
    pub request: String,
    /// Expected HTTP response block.
    pub expected_response: String,
    /// Response match mode used by the execution engine.
    #[serde(default)]
    pub response_match: ResponseMatchMode,
    /// Response fields documented as intentionally ignored during strict matching.
    #[serde(default)]
    pub response_ignore: Vec<String>,
    /// Optional JSON Schema block used when `response.match: schema`.
    #[serde(default)]
    pub response_schema: Option<String>,
    /// Optional agent task test instructions.
    pub tests: Option<String>,
    /// Optional notes section.
    pub notes: Option<String>,
    /// Structured assertions from the `## Assertions` section.
    #[serde(default)]
    pub assertions: Vec<Assertion>,
}

/// A structured assertion rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assertion {
    /// Path to check: `"status"`, `"body.field"`, `"headers.content-type"`, etc.
    pub path: String,
    /// Comparison operator.
    pub op: AssertionOp,
    /// Value operand (absent for `exists`).
    pub value: Option<String>,
}

/// Assertion operator.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AssertionOp {
    /// Field exists and is non-null.
    Exists,
    /// Exact string match.
    Equals,
    /// Substring match.
    Contains,
    /// Regex match.
    Matches,
    /// Value is one of a comma-separated list.
    In,
}

/// Endpoint YAML frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct EndpointSchema {
    /// Resource group, usually matching the containing folder.
    pub resource: String,
    /// Protocol. Reqbook v1.0 executes HTTP only.
    pub protocol: Protocol,
    /// HTTP method.
    pub method: HttpMethod,
    /// URL path, with path params in `:param` form.
    pub path: String,
    /// Search and grouping tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Endpoint spec version.
    pub version: u32,
    /// Supported environments.
    #[serde(default)]
    pub env: Vec<String>,
    /// Auth mode.
    #[serde(default)]
    pub auth: Option<AuthMode>,
    /// Timeout in milliseconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Retry policy.
    #[serde(default)]
    pub retry: Option<RetryPolicy>,
    /// Response comparison configuration.
    #[serde(default)]
    pub response: Option<ResponseConfig>,
}

/// Response comparison configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct ResponseConfig {
    /// Match mode. Defaults to shape matching.
    #[serde(default, rename = "match")]
    pub match_mode: Option<ResponseMatchMode>,
    /// Paths to ignore in strict mode, e.g. `body.id` or `headers.x-request-id`.
    #[serde(default)]
    pub ignore: Vec<String>,
}

/// Response comparison modes.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ResponseMatchMode {
    /// JSON shape matching; scalar values are type-checked, not exact-matched.
    #[default]
    Shape,
    /// Exact status, expected headers, and exact JSON/string body matching.
    Strict,
    /// Validate the actual response body against a JSON Schema block.
    Schema,
}

/// Supported endpoint protocols.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    /// HTTP request/response.
    Http,
    /// Reserved for future versions.
    Ws,
    /// Reserved for future versions.
    Sse,
}

/// HTTP method supported by request blocks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    /// GET.
    Get,
    /// POST.
    Post,
    /// PUT.
    Put,
    /// PATCH.
    Patch,
    /// DELETE.
    Delete,
    /// HEAD.
    Head,
    /// OPTIONS.
    Options,
}

impl HttpMethod {
    /// Return the uppercase method string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }
}

/// Endpoint auth modes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    /// No auth.
    None,
    /// Bearer token auth.
    Bearer,
    /// Basic auth.
    Basic,
    /// Custom auth handled by request headers.
    Custom,
}

/// Retry policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Number of retry attempts after the first request.
    pub attempts: u32,
    /// Backoff mode.
    pub backoff: Backoff,
}

/// Retry backoff mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Backoff {
    /// Fixed delay.
    Fixed,
    /// Exponential delay.
    Exponential,
}

/// Parsed pipeline specification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pipeline {
    /// Source path when loaded from disk.
    pub source: Option<String>,
    /// Pipeline frontmatter.
    pub schema: PipelineSchema,
    /// Pipeline title.
    pub title: String,
    /// Ordered steps.
    pub steps: Vec<PipelineStep>,
}

/// Pipeline YAML frontmatter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct PipelineSchema {
    /// Must be `pipeline`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Pipeline name.
    pub name: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// Whether execution continues after step failure.
    #[serde(default)]
    pub continue_on_error: bool,
    /// Whether independent steps may run concurrently.
    #[serde(default)]
    pub parallel: bool,
}

/// One pipeline step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PipelineStep {
    /// Step display name.
    pub name: String,
    /// Endpoint path relative to `api-docs/`.
    pub endpoint: String,
    /// Variable names injected into the step.
    pub inject: Vec<String>,
    /// Captures from response paths.
    pub capture: Vec<Capture>,
    /// Assertions in simple textual form.
    pub assert: Vec<String>,
}

/// A captured pipeline variable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Capture {
    /// JSONPath-like source expression.
    pub source: String,
    /// Destination variable name.
    pub name: String,
}

/// Environment config keyed by environment name.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct EnvConfig {
    /// Environment values.
    pub envs: BTreeMap<String, BTreeMap<String, String>>,
}
