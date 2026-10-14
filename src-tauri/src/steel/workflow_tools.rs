//! Bounded schemas. Starting or versioning workflows uses existing permission review.
use super::agent_tools::{gateway_tool, SteelAgentGateway};
use crate::agent::{protocol::PermissionRisk, runtime::ToolRegistration};
use serde_json::{json, Value};
use std::sync::Arc;
pub(super) fn registrations(gateway: Arc<dyn SteelAgentGateway>) -> Vec<ToolRegistration> {
    let selection = json!({"type":"object","properties":{"templateId":{"type":"string"},"templateRevision":{"type":"integer","minimum":1,"maximum":256},"projectId":{"type":"string"},"datasetId":{"type":"string"},"datasetSha256":{"type":"string","minLength":64,"maxLength":64}},"required":["templateId","templateRevision","projectId","datasetId","datasetSha256"],"additionalProperties":false});
    let mut start = selection.clone();
    for (key, schema) in [
        (
            "dryRunSha256",
            json!({"type":"string","minLength":64,"maxLength":64}),
        ),
        ("nonce", json!({"type":"string"})),
        ("confirmed", json!({"type":"boolean","const":true})),
    ] {
        start["properties"][key] = schema;
        start["required"].as_array_mut().unwrap().push(json!(key));
    }
    let paging = json!({"type":"object","properties":{"limit":{"type":"integer","minimum":1,"maximum":50},"offset":{"type":"integer","minimum":0,"maximum":10000}},"additionalProperties":false});
    let specs: [(&str, &str, Value, PermissionRisk, bool); 7] = [
        (
            "research.list_workflow_templates",
            "research_list_workflow_templates",
            paging.clone(),
            PermissionRisk::Automatic,
            true,
        ),
        (
            "research.get_workflow_template",
            "research_get_workflow_template",
            json!({"type":"object","properties":{"id":{"type":"string"},"revision":{"type":"integer","minimum":1,"maximum":256}},"required":["id","revision"],"additionalProperties":false}),
            PermissionRisk::Automatic,
            true,
        ),
        (
            "research.save_workflow_template",
            "research_save_workflow_template",
            json!({"type":"object","properties":{"template":{"type":"object"},"expectedRevision":{"type":"integer","minimum":0,"maximum":255}},"required":["template","expectedRevision"],"additionalProperties":false}),
            PermissionRisk::ConfirmationRequired,
            false,
        ),
        (
            "research.preview_workflow",
            "research_preview_workflow",
            selection,
            PermissionRisk::Automatic,
            true,
        ),
        (
            "research.start_workflow",
            "research_start_workflow",
            start,
            PermissionRisk::ConfirmationRequired,
            false,
        ),
        (
            "research.list_workflows",
            "research_list_workflows",
            paging,
            PermissionRisk::Automatic,
            true,
        ),
        (
            "research.get_workflow",
            "research_get_workflow",
            json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false}),
            PermissionRisk::Automatic,
            true,
        ),
    ];
    specs
        .into_iter()
        .map(|(id, name, schema, risk, read_only)| {
            gateway_tool(id, name, schema, risk, read_only, gateway.clone())
        })
        .collect()
}
