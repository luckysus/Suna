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
