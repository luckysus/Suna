//! Opt-in bibliographic adapters reuse the existing cancellable task scheduler.
use crate::literature_metadata::provider;
pub use crate::literature_metadata::{parse_crossref, ProviderId, Search};
use crate::{
    db::{current_workspace_id, with_conn, with_conn_mut, DbState},
    storage::repositories::{literature_discovery as repo, research_artifacts as artifacts},
    tasks::{
        repository,
        scheduler::{HandlerContext, HandlerError, HandlerFuture, HandlerOutcome, TaskHandler},
        NewTask, TaskRecord,
    },
};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::{path::PathBuf, time::Duration};
mod persistence;
pub(crate) mod quality;
fn validate(s: &Search) -> Result<(), String> {
    if s.query.trim().is_empty()
        || s.query.len() > 1000
        || s.cursor.as_ref().is_some_and(|c| c.len() > 4096)
        || s.from_year.is_some_and(|y| !(1000..=2200).contains(&y))
        || s.to_year.is_some_and(|y| !(1000..=2200).contains(&y))
        || matches!((s.from_year,s.to_year),(Some(a),Some(b))if a>b)
    {
        return Err("invalid_literature_search".into());
    }
    Ok(())
}
fn submit(c: &Connection, ws: &str, s: Search) -> Result<TaskRecord, String> {
    validate(&s)?;
    if !repo::online(c, ws)? {
        return Err("literature_network_disabled".into());
    }
    repository::create(
        c,
        NewTask {
            workspace_id: ws.into(),
            kind: "literature_metadata_search".into(),
            payload_json: json!(s).to_string(),
            checkpoint_json: None,
            next_run_at: None,
            progress: 0,
        },
    )
    .map_err(|e| e.to_string())
}
pub struct MetadataSearchHandler {
    pub database: PathBuf,
    #[cfg(test)]
    endpoint: Option<String>,
}
impl MetadataSearchHandler {
    pub fn new(database: PathBuf) -> Self {
        Self {
            database,
            #[cfg(test)]
            endpoint: None,
        }
    }
}
impl TaskHandler for MetadataSearchHandler {
    fn kind(&self) -> &str {
        "literature_metadata_search"
    }
    fn resumable(&self) -> bool {
        true
    }
    fn run(&self, task: TaskRecord, context: HandlerContext) -> HandlerFuture {
        let db = self.database.clone();
        let source: Search = match serde_json::from_str(&task.payload_json) {
            Ok(source) => source,
            Err(_) => {
                return Box::pin(async {
                    Err(HandlerError::permanent("invalid_literature_search"))
                })
            }
        };
        let endpoint = provider(source.provider).endpoint().to_string();
        #[cfg(test)]
        let endpoint = self.endpoint.clone().unwrap_or(endpoint);
        Box::pin(async move { execute(task, context, db, endpoint).await })
    }
}
async fn execute(
    task: TaskRecord,
    context: HandlerContext,
    db: PathBuf,
    endpoint: String,
) -> Result<HandlerOutcome, HandlerError> {
    let s: Search = serde_json::from_str(&task.payload_json)
        .map_err(|_| HandlerError::permanent("invalid_literature_search"))?;
    validate(&s).map_err(HandlerError::permanent)?;
    let (c, _) = crate::storage::database::open(&db)
        .map_err(|_| HandlerError::retryable("storage_unavailable"))?;
    let reserve = repo::reserve_for_provider(
        &c,
        &task.workspace_id,
        s.provider,
        chrono::Utc::now().timestamp_millis(),
    );
    if let Err(e) = reserve {
        return Err(if e == "literature_rate_limited" {
            HandlerError::retryable(e)
        } else {
            HandlerError::permanent(e)
        });
    }
    drop(c);
    context
        .checkpoint(Some("{\"stage\":\"public_metadata_request\"}"), 10, None)
        .map_err(|_| HandlerError::retryable("checkpoint_failed"))?;
    let request = async {
        let client = crate::providers::http::build_no_redirect_client(
            &crate::providers::http::HttpClientConfig {
                connect_timeout: Duration::from_secs(10),
                request_timeout: Duration::from_secs(15),
                proxy_url: None,
            },
        )
        .map_err(|_| HandlerError::permanent("literature_client_failed"))?;
        let adapter = provider(s.provider);
        let q = adapter.parameters(&s);
        let mut response = client
            .get(&endpoint)
            .query(&q)
            .send()
            .await
            .map_err(|_| HandlerError::retryable("literature_provider_unavailable"))?;
        if response.status().as_u16() == 429 {
            return Err(HandlerError::retryable("literature_rate_limited"));
        }
        if !response.status().is_success() {
            return Err(HandlerError::retryable("literature_http_error"));
        }
        let mut bytes = vec![];
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| HandlerError::retryable("literature_response_failed"))?
        {
            if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
                return Err(HandlerError::permanent("literature_response_limit"));
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes)
            .map_err(|_| HandlerError::permanent("literature_invalid_response"))?;
        adapter.parse(&value).map_err(HandlerError::permanent)
    };
    let stop = async {
        loop {
            if context.cancellation_requested().unwrap_or(true) || context.shutdown_requested() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            let enabled = crate::storage::database::open(&db)
                .ok()
                .and_then(|(c, _)| repo::online(&c, &task.workspace_id).ok())
                .unwrap_or(false);
            if !enabled {
                return;
            }
        }
    };
    let (records, cursor) =
        tokio::select! {r=request=>r?,_=stop=>return Ok(HandlerOutcome::Cancelled)};
    if context.cancellation_requested().unwrap_or(true) {
        return Ok(HandlerOutcome::Cancelled);
    }
    let (mut c, _) = crate::storage::database::open(&db)
        .map_err(|_| HandlerError::retryable("storage_unavailable"))?;
    let Some(saved) = persistence::store_page(&mut c, &task.workspace_id, records)
        .map_err(persistence::handler_error)?
    else {
        return Ok(HandlerOutcome::Cancelled);
    };
    let result = json!({"records":saved,"nextCursor":cursor,"provider":s.provider,"sampleScope":"user_search_results","query":s.query,"fromYear":s.from_year,"toYear":s.to_year});
    context
        .checkpoint(
            Some(&json!({"result":result,"stage":"completed"}).to_string()),
            100,
            None,
        )
        .map_err(|_| HandlerError::retryable("checkpoint_failed"))?;
    Ok(HandlerOutcome::Completed)
}
#[tauri::command]
pub fn search_public_literature(
    db: tauri::State<DbState>,
    request: Search,
) -> Result<crate::app::task_commands::tasks::BackgroundTaskResponse, String> {
    with_conn(&db, |c| {
        submit(c, current_workspace_id(), request)
            .map(crate::app::task_commands::tasks::background_task_response)
    })
}
#[tauri::command]
pub fn list_discovered_literature(
    db: tauri::State<DbState>,
    query: String,
    offset: Option<u32>,
) -> Result<Vec<repo::Record>, String> {
    with_conn(&db, |c| {
        repo::list(c, current_workspace_id(), &query, offset.unwrap_or(0))
    })
}
#[tauri::command]
pub fn get_literature_discovery_result(
    db: tauri::State<DbState>,
    id: String,
) -> Result<Option<Value>, String> {
    with_conn(&db, |c| {
        let id = uuid::Uuid::parse_str(&id).map_err(|e| e.to_string())?;
        let t = repository::get(c, current_workspace_id(), id)
            .map_err(|e| e.to_string())?
            .ok_or("task_not_found")?;
        if t.kind != "literature_metadata_search" {
            return Err("not_literature_search_task".into());
        }
        if t.state != crate::tasks::TaskState::Completed {
            return Ok(None);
        }
        let body: Value =
            serde_json::from_str(t.checkpoint_json.as_deref().ok_or("missing_result")?)
                .map_err(|e| e.to_string())?;
        Ok(Some(body["result"].clone()))
    })
}
#[tauri::command]
pub fn attach_discovered_literature(
    db: tauri::State<DbState>,
    project_id: String,
    id: String,
) -> Result<(), String> {
    with_conn(&db, |c| {
        repo::attach_metadata(c, current_workspace_id(), &project_id, &id)
    })
}
#[tauri::command]
pub fn link_literature_fulltext(
    db: tauri::State<DbState>,
    project_id: String,
    id: String,
    document_id: String,
    version_id: String,
    rights_basis: String,
) -> Result<(), String> {
    with_conn_mut(&db, |c| {
        repo::link_fulltext(
            c,
            current_workspace_id(),
            &project_id,
            &id,
            &document_id,
            &version_id,
            &rights_basis,
        )
    })
}
#[tauri::command]
pub fn archive_reviewed_literature_evidence(
    db: tauri::State<DbState>,
    project_id: String,
    request: repo::Evidence,
) -> Result<String, String> {
    with_conn(&db, |c| {
        repo::save_evidence(c, current_workspace_id(), &project_id, request)
    })
}
#[tauri::command]
pub fn get_project_literature_evidence(
    db: tauri::State<DbState>,
    project_id: String,
) -> Result<Vec<Value>, String> {
    with_conn(&db, |c| {
        repo::evidence_rows(c, current_workspace_id(), &project_id)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn public_metadata_retains_unknowns_without_inventing_facts() {
        let (r,cursor)=parse_crossref(&json!({"message":{"items":[{"DOI":"https://doi.org/10.1234/ABC","title":["Synthetic API fixture"],"author":[{"family":"Fixture"}],"published":{"date-parts":[[2020]]}}],"next-cursor":"cursor"}})).unwrap();
        assert_eq!(r[0].doi.as_deref(), Some("10.1234/abc"));
        assert_eq!(r[0].year, Some(2020));
        assert_eq!(r[0].fulltext_state, "link_only");
        assert!(r[0].license.is_none());
        assert!(r[0].abstract_text.is_none());
        assert_eq!(cursor.as_deref(), Some("cursor"));
    }
    #[test]
    fn opt_in_is_required_and_bad_search_never_creates_task() {
        let mut c = Connection::open_in_memory().unwrap();
        crate::storage::migrations::migrate(&mut c).unwrap();
        let valid = Search {
            provider: ProviderId::Crossref,
            query: "steel".into(),
            cursor: None,
            from_year: None,
            to_year: None,
        };
        assert!(submit(&c, "local", valid.clone())
            .unwrap_err()
            .contains("network_disabled"));
        assert!(repository::list(&c, "local").unwrap().is_empty());
        crate::storage::repositories::settings::set(&mut c, "local", repo::NETWORK_KEY, "true")
            .unwrap();
        let task = submit(&c, "local", valid).unwrap();
        assert_eq!(task.kind, "literature_metadata_search");
        assert!(submit(
            &c,
            "local",
            Search {
                provider: ProviderId::Crossref,
                query: "steel".into(),
                cursor: None,
                from_year: Some(2020),
                to_year: Some(2010)
            }
        )
        .is_err());
    }
}

#[tauri::command]
pub fn search_local_literature_evidence(
    db: tauri::State<DbState>,
    query: String,
) -> Result<Value, String> {
    with_conn(&db, |c| {
        if query.trim().is_empty() || query.len() > 1000 {
            return Err("invalid_evidence_query".into());
        }
        let hits = crate::rag::index::fts::search(
            c,
            &crate::rag::index::fts::FtsSearchRequest {
                workspace_id: current_workspace_id().into(),
                query,
                knowledge_base_ids: vec![],
                limit: 20,
            },
        )
        .map_err(|e| e.to_string())?;
        let mut rows = vec![];
        for h in hits {
            let location:String=c.query_row("SELECT source_location_json FROM knowledge_chunks WHERE workspace_id=?1 AND version_id=?2 AND id=?3",rusqlite::params![current_workspace_id(),h.version_id.to_string(),h.chunk_id.to_string()],|r|r.get(0)).map_err(|e|e.to_string())?;
            rows.push(json!({"documentId":h.document_id,"versionId":h.version_id,"chunkId":h.chunk_id,"source":h.source_name,"quote":h.text,"location":serde_json::from_str::<Value>(&location).map_err(|e|e.to_string())?}));
        }
        Ok(json!(rows))
    })
}
#[tauri::command]
pub fn create_literature_evidence_dataset(
    app: tauri::AppHandle,
    db: tauri::State<DbState>,
    project_id: String,
) -> Result<crate::storage::repositories::steel::SteelDatasetRecord, String> {
    let root = crate::db::app_data_directory(&app)?;
    with_conn(&db, |c| {
        repo::evidence_dataset(c, current_workspace_id(), &project_id, &root)
    })
}

#[cfg(test)]
mod network_tests {
    use super::*;
    use crate::tasks::{
        scheduler::{EventSink, Scheduler, SchedulerConfig, SchedulerEvent, SystemClock},
        TaskState,
    };
    use std::{sync::Arc, time::Instant};
    struct Sink;
    impl EventSink for Sink {
        fn emit(&self, _: SchedulerEvent) {}
    }
    #[test]
    #[ignore = "explicit public Crossref network smoke; does not touch user databases"]
    fn real_crossref_search_persists_metadata_and_remains_readable_offline() {
        let root = std::env::temp_dir().join(format!("suna-crossref-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let db = root.join("fixture.sqlite");
        let (mut c, _) = crate::storage::database::open(&db).unwrap();
        crate::storage::repositories::settings::set(&mut c, "local", repo::NETWORK_KEY, "true")
            .unwrap();
        let task = submit(
            &c,
            "local",
            Search {
                provider: ProviderId::Crossref,
                query: "martensitic steel".into(),
                cursor: None,
                from_year: None,
                to_year: None,
            },
        )
        .unwrap();
        let mut s = Scheduler::new(
            db.clone(),
            "local".into(),
            SchedulerConfig {
                max_attempts: 1,
                ..Default::default()
            },
            Arc::new(SystemClock),
            vec![Arc::new(MetadataSearchHandler::new(db))],
            Arc::new(Sink),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(25);
        loop {
            s.tick().unwrap();
            let state = repository::get(&c, "local", task.id).unwrap().unwrap();
            if matches!(
                state.state,
                TaskState::Completed | TaskState::Failed | TaskState::Cancelled
            ) {
                assert_eq!(state.state, TaskState::Completed, "{:?}", state.error_code);
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        let rows = repo::list(&c, "local", "", 0).unwrap();
        assert!(!rows.is_empty());
        assert!(rows
            .iter()
            .all(|r| r.metadata.fulltext_state == "link_only"));
        crate::storage::repositories::settings::set(&mut c, "local", repo::NETWORK_KEY, "false")
            .unwrap();
        assert!(!repo::online(&c, "local").unwrap());
        assert_eq!(repo::list(&c, "local", "", 0).unwrap().len(), rows.len());
        drop(s);
        drop(c);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod fixture_http_tests {
    use super::*;
    use crate::tasks::{
        scheduler::{EventSink, Scheduler, SchedulerConfig, SchedulerEvent, SystemClock},
        TaskState,
    };
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        time::Instant,
    };
    struct Sink;
    impl EventSink for Sink {
        fn emit(&self, _: SchedulerEvent) {}
    }
    fn run_provider_http_fixture(source: ProviderId) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/works", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut buf = [0; 8192];
            let n = socket.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..n]);
            assert!(request.contains("query=synthetic"));
            let target = request
                .lines()
                .next()
                .unwrap()
                .split_whitespace()
                .nth(1)
                .unwrap();
            let url = reqwest::Url::parse(&format!("http://127.0.0.1{target}")).unwrap();
            let query: std::collections::HashMap<String, String> =
                url.query_pairs().into_owned().collect();
            assert_eq!(query.get("query").unwrap(), "synthetic 钢+?");
            match source {
                ProviderId::Crossref => {
                    assert_eq!(query["rows"], "20");
                    assert_eq!(query["cursor"], "*");
                }
                ProviderId::EuropePmc => {
                    assert_eq!(query["pageSize"], "20");
                    assert_eq!(query["cursorMark"], "*");
                    assert_eq!(query["resultType"], "core");
                    assert_eq!(query["format"], "json");
                }
            }
            let body=match source {
                ProviderId::Crossref=>json!({"message":{"items":[{"DOI":"10.1234/fixture","title":["Synthetic local HTTP provider fixture"],"published":{"date-parts":[[2020]]}}],"next-cursor":"next"}}),
                ProviderId::EuropePmc=>json!({"resultList":{"result":[{"source":"MED","id":"1234","title":"Synthetic EU HTTP provider fixture","doi":"10.1234/fixture","pubYear":"2020","isOpenAccess":"Y"}]},"nextCursorMark":"next"}),
            }.to_string();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body).unwrap();
        });
        let root =
            std::env::temp_dir().join(format!("suna-metadata-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let db = root.join("fixture.sqlite");
        let (mut c, _) = crate::storage::database::open(&db).unwrap();
        let p = crate::storage::repositories::research_projects::save(
            &mut c,
            "local",
            crate::storage::repositories::research_projects::SaveResearchProject {
                id: None,
                name: "合成接口验证".into(),
                description: "".into(),
                material_system: None,
                tags: vec![],
                expected_updated_at: None,
            },
        )
        .unwrap();
        crate::storage::repositories::settings::set(
            &mut c,
            "local",
            "suna.research.current_project",
            &json!(p.id).to_string(),
        )
        .unwrap();
        crate::storage::repositories::settings::set(&mut c, "local", repo::NETWORK_KEY, "true")
            .unwrap();
        let t = submit(
            &c,
            "local",
            Search {
                provider: source,
                query: "synthetic 钢+?".into(),
                cursor: None,
                from_year: None,
                to_year: None,
            },
        )
        .unwrap();
        let handler = MetadataSearchHandler {
            database: db.clone(),
            endpoint: Some(endpoint),
        };
        let mut scheduler = Scheduler::new(
            db.clone(),
            "local".into(),
            SchedulerConfig {
                max_attempts: 1,
                ..Default::default()
            },
            Arc::new(SystemClock),
            vec![Arc::new(handler)],
            Arc::new(Sink),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            scheduler.tick().unwrap();
            let task = repository::get(&c, "local", t.id).unwrap().unwrap();
            if matches!(task.state, TaskState::Completed | TaskState::Failed) {
                assert_eq!(task.state, TaskState::Completed, "{:?}", task.error_code);
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
        server.join().unwrap();
        let rows = repo::list(&c, "local", "", 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].metadata.provider, source.as_str());
        assert_eq!(rows[0].metadata.fulltext_state, "link_only");
        assert!(rows[0].metadata.license.is_none());
        let task = repository::get(&c, "local", t.id).unwrap().unwrap();
        let checkpoint: Value =
            serde_json::from_str(task.checkpoint_json.as_deref().unwrap()).unwrap();
        assert_eq!(checkpoint["result"]["provider"], source.as_str());
        assert_eq!(checkpoint["result"]["nextCursor"], "next");
        assert_eq!(rows[0].metadata.doi.as_deref(), Some("10.1234/fixture"));
        let outputs = artifacts::list(&c, "local", &p.id, 25, 0).unwrap();
        assert_eq!(outputs.len(), 1);
        let frozen = artifacts::get(&c, "local", &outputs[0].id).unwrap();
        let frozen_payload = frozen.payload.as_ref().unwrap();
        assert_eq!(frozen_payload["provider"], source.as_str());
        assert_eq!(frozen_payload["records"][0]["literatureId"], rows[0].id);
        assert_eq!(
            frozen_payload["records"][0]["revisionId"],
            rows[0].revision_id
        );
        assert_eq!(
            frozen_payload["records"][0]["metadataSha256"],
            rows[0].revision_sha256
        );
        use sha2::{Digest, Sha256};
        assert_eq!(
            frozen_payload["querySha256"],
            format!(
                "{:x}",
                Sha256::digest(json!("synthetic 钢+?").to_string().as_bytes())
            )
        );
        artifacts::reconcile_task(&c, "local", &t.id.to_string()).unwrap();
        assert_eq!(
            artifacts::get(&c, "local", &frozen.id)
                .unwrap()
                .content_sha256,
            frozen.content_sha256
        );
        crate::storage::repositories::settings::set(&mut c, "local", repo::NETWORK_KEY, "false")
            .unwrap();
        assert_eq!(repo::list(&c, "local", "", 0).unwrap().len(), 1);
        drop(scheduler);
        drop(c);
        let (c, _) = crate::storage::database::open(&db).unwrap();
        assert!(!repo::online(&c, "local").unwrap());
        let reopened = repo::list(&c, "local", "", 0).unwrap();
        assert_eq!(reopened[0].metadata.provider, source.as_str());
        assert_eq!(reopened[0].revision_sha256, rows[0].revision_sha256);
        let reopened_output = artifacts::get(&c, "local", &frozen.id).unwrap();
        assert_eq!(reopened_output.content_sha256, frozen.content_sha256);
        assert_eq!(reopened_output.payload, frozen.payload);
        drop(c);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn scheduler_real_http_fixture_then_offline_cache_and_project_source() {
        run_provider_http_fixture(ProviderId::Crossref);
    }
    #[test]
    fn european_provider_uses_real_scheduler_http_storage_and_offline_reopen() {
        run_provider_http_fixture(ProviderId::EuropePmc);
    }
}
