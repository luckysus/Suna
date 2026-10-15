use super::context::SaveContext;
use serde_json::json;

#[test]
fn desktop_context_contract_rejects_credentials_self_approval_and_measurements() {
    let request = json!({"evidenceArtifactId":"00000000-0000-4000-8000-000000000001","evidenceSha256":"a".repeat(64),"confirmed":true,
        "conditions":[{"kind":"test_method","text":"ASTM E8","quote":"Synthetic test used ASTM E8.","chunkId":"chunk-synthetic","chunkSha256":"b".repeat(64)}]});
    assert!(serde_json::from_value::<SaveContext>(request.clone()).is_ok());
    for field in [
        "api_key",
        "password",
        "permit",
        "autoTrain",
        "measurement",
        "executable",
    ] {
        let mut extra = request.clone();
        extra[field] = json!("synthetic-not-accepted");
        assert!(
            serde_json::from_value::<SaveContext>(extra).is_err(),
            "{field}"
        );
    }
    let mut extra = request;
    extra["conditions"][0]["api_key"] = json!("synthetic-not-accepted");
    assert!(serde_json::from_value::<SaveContext>(extra).is_err());
}
