//! Request/response data types for the preview API.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub(super) const API_DOCS_DIR: &str = "api-docs";
pub(super) const APIS_DIR: &str = "apis";
pub(super) const FLOWS_DIR: &str = "flows";
pub(super) const LEGACY_FLOWS_DIR: &str = "pipelines";

#[derive(Debug, Clone, Serialize)]
pub(super) struct SpecEntry {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) title: String,
    pub(super) rel_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ResourceGroup {
    pub(super) resource: String,
    pub(super) specs: Vec<SpecEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct IndexResponse {
    pub(super) project_name: String,
    pub(super) groups: Vec<ResourceGroup>,
    pub(super) spec_count: usize,
    pub(super) version: &'static str,
    pub(super) mock_mode: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct SpecResponse {
    pub(super) title: String,
    pub(super) method: String,
    pub(super) path: String,
    pub(super) description: String,
    pub(super) request: String,
    pub(super) expected_response: String,
    pub(super) tests: Option<String>,
    pub(super) rel_path: String,
    pub(super) env: String,
    pub(super) raw_source: String,
    pub(super) version: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FlowEntry {
    pub(super) name: String,
    pub(super) title: String,
    pub(super) rel_path: String,
    pub(super) steps: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FlowsResponse {
    pub(super) flows: Vec<FlowEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FlowResponse {
    pub(super) name: String,
    pub(super) title: String,
    pub(super) description: Option<String>,
    pub(super) rel_path: String,
    pub(super) raw_source: String,
    pub(super) steps: Vec<FlowStepResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FlowStepResponse {
    pub(super) name: String,
    pub(super) endpoint: String,
    pub(super) inject: Vec<String>,
    pub(super) capture: Vec<FlowCaptureResponse>,
    pub(super) assert: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct FlowCaptureResponse {
    pub(super) source: String,
    pub(super) name: String,
}

#[derive(Debug, Deserialize, Default)]
pub(super) struct ExecBody {
    #[serde(default)]
    pub(super) vars: BTreeMap<String, String>,
    #[serde(default, alias = "params")]
    pub(super) path_params: BTreeMap<String, String>,
    #[serde(default)]
    pub(super) headers: BTreeMap<String, String>,
    pub(super) body: Option<String>,
}

#[derive(Debug, Default)]
pub(super) struct RuntimeOverrides {
    pub(super) vars: BTreeMap<String, String>,
    pub(super) path_params: BTreeMap<String, String>,
    pub(super) headers: BTreeMap<String, String>,
    pub(super) body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SaveVarsBody {
    pub(super) env: Option<String>,
    #[serde(default)]
    pub(super) vars: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ScanRoute {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) title: String,
    pub(super) resource: String,
    pub(super) exists: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ScanProjectResponse {
    pub(super) project_name: String,
    pub(super) routes_found: usize,
    pub(super) missing_count: usize,
    pub(super) existing_count: usize,
    pub(super) duration_ms: u128,
    pub(super) routes: Vec<ScanRoute>,
    pub(super) written: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct ValidateResponse {
    pub(super) valid: bool,
    pub(super) kind: String,
    pub(super) path: String,
    pub(super) error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct GitBranchEntry {
    pub(super) name: String,
    pub(super) current: bool,
    pub(super) remote: bool,
    pub(super) upstream: Option<String>,
    pub(super) commit: Option<String>,
    pub(super) summary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct GitBranchesResponse {
    pub(super) is_repo: bool,
    pub(super) root: Option<String>,
    pub(super) current: Option<String>,
    pub(super) dirty: bool,
    pub(super) branches: Vec<GitBranchEntry>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AdHocReqBody {
    pub(super) method: String,
    pub(super) url: String,
    #[serde(default)]
    pub(super) headers: BTreeMap<String, String>,
    pub(super) body: Option<String>,
    #[serde(default)]
    pub(super) vars: BTreeMap<String, String>,
    #[serde(default = "default_env")]
    pub(super) env: String,
    pub(super) save_as: Option<String>,
}

pub(super) fn default_env() -> String {
    "dev".to_string()
}

#[derive(Debug, Serialize)]
pub(super) struct AdHocReqResponse {
    #[serde(flatten)]
    pub(super) execution: crate::engine::Execution,
    pub(super) saved_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct OpenWorkspaceBody {
    pub(super) path: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateWorkspaceBody {
    pub(super) path: String,
    pub(super) name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct CheckoutBranchBody {
    pub(super) branch: String,
}
