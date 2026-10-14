use serde_json::json;
use suna::storage::repositories::research_artifacts::{safe_metadata, safe_request};
#[test]
fn provenance_allowlists_drop_credentials_raw_rows_and_private_paths() {
    let hostile = json!({"metrics":{"rmse":1.0,"api_key":"fake-key"},"api_key":"fake-key","password":"fake-password","access_token":"fake-token","source_path":"C:/private/data.csv","features":[[1,2]],"seed":42});
    for safe in [safe_metadata(&hostile), safe_request(&hostile)] {
        let serialized = safe.to_string();
        for private in ["fake-key", "fake-password", "fake-token", "C:/private"] {
            assert!(!serialized.contains(private));
        }
    }
    assert_eq!(safe_metadata(&hostile)["metrics"]["rmse"], 1.0);
    assert_eq!(safe_request(&hostile)["seed"], 42);
}

#[test]
fn applicability_evidence_is_preserved_without_copying_batch_inputs() {
    let result = json!({"input_rows":[[987.654]],"applicability":{"schema_version":"applicability-support.v1","method":"marginal_training_ranges","model_id":"model","support_sha256":null,"probability":null,"physical_feasibility":"not_assessed","joint_support":"not_assessed","rows":[{"row_index":0,"state":"unknown","reasons":[{"state":"unknown","code":"missing_unit","feature":"C","value":987.654,"source_path":"C:/private","password":"sentinel-password"}]}]}});
    let safe = safe_metadata(&result);
    assert_eq!(safe["applicability"]["rows"][0]["state"], "unknown");
    assert_eq!(
        safe["applicability"]["rows"][0]["reasons"][0]["feature"],
        "C"
    );
    for private in ["987.654", "sentinel-password", "C:/private", "input_rows"] {
        assert!(!safe.to_string().contains(private));
    }
}
