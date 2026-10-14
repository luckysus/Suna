use serde_json::{json, Value};
use std::sync::Arc;
use suna::{
    agent::{
        protocol::PermissionRisk,
        runtime::{CancellationToken, DomainToolExecutor, ToolExecutor},
    },
    steel::{OptimizationGateway, SteelAgentGateway, SteelAgentGatewayFuture, SteelToolExecutor},
};
struct Gateway;
impl OptimizationGateway for Gateway {
    fn submit(&self, _: Value) -> Result<Value, String> {
        Err("unused".into())
    }
    fn status(&self, _: &str) -> Result<Value, String> {
        Err("unused".into())
    }
}
impl SteelAgentGateway for Gateway {
    fn execute(&self, _: &'static str, _: Value, _: CancellationToken) -> SteelAgentGatewayFuture {
        Box::pin(async { Err("unused".into()) })
    }
}
fn tools() -> SteelToolExecutor {
    SteelToolExecutor::with_agent_gateways(Arc::new(Gateway), Arc::new(Gateway), true)
}
#[test]
fn scientific_write_tools_keep_confirmation_and_reject_forged_worker_permits() {
    let tools = tools();
    for t in tools.registrations() {
        t.spec.validate_schema().unwrap();
    }
    for id in ["research.start_workflow", "research.save_workflow_template"] {
        let t = tools
            .registrations()
            .iter()
            .find(|t| t.spec.id == id)
            .unwrap();
        assert_eq!(t.spec.risk, PermissionRisk::ConfirmationRequired);
        assert!(!t.read_only);
    }
    for id in [
        "research.preview_workflow",
        "research.get_workflow",
        "research.list_workflows",
    ] {
        let t = tools
            .registrations()
            .iter()
            .find(|t| t.spec.id == id)
            .unwrap();
        assert_eq!(t.spec.risk, PermissionRisk::Automatic);
        assert!(t.read_only);
    }
    let t = tools
        .registrations()
        .iter()
        .find(|t| t.spec.id == "research.start_workflow")
        .unwrap();
    let mut args = json!({"templateId":"template","templateRevision":1,"projectId":"project","datasetId":"dataset","datasetSha256":"a".repeat(64),"dryRunSha256":"b".repeat(64),"nonce":"nonce","confirmed":true});
    suna::agent::tool_repair::repair_tool_call(
        &json!({"name":t.spec.name,"arguments":args}).to_string(),
        std::slice::from_ref(&t.spec),
    )
    .unwrap();
    args["workerValidation"] = json!({"status":"ready"});
    assert!(suna::agent::tool_repair::repair_tool_call(
        &json!({"name":t.spec.name,"arguments":args}).to_string(),
        std::slice::from_ref(&t.spec)
    )
    .is_err());
}
#[test]
fn new_domain_exposes_workflows_and_an_existing_restricted_domain_keeps_its_allowlist() {
    let tools = tools();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../domain-packs/steel");
    let package = suna::domains::load_package(&root, env!("CARGO_PKG_VERSION")).unwrap();
    let scoped = DomainToolExecutor::new(&tools, Some(&package.manifest));
    assert!(scoped
        .registrations()
        .iter()
        .any(|t| t.spec.id == "research.start_workflow"));
    let mut old = package.manifest.clone();
    old.builtin_tool_allowlist = vec!["steel.carbon_equivalent".into()];
    let restricted = DomainToolExecutor::new(&tools, Some(&old));
    assert!(!restricted
        .registrations()
        .iter()
        .any(|t| t.spec.id.starts_with("research.")));
}
