use super::*;
use crate::knowledge_db::live_tests::{configured_pool, fixture_schema};
use crate::rag::chunk::{chunk_document, ChunkPolicy};
use crate::rag::ingest::{ingest_file, IngestLimits};
use crate::rag::parse::{parse_document, ParseLimits};

#[test]
#[ignore = "requires an actual OA literature file and configured local PostgreSQL; fixture transaction is rolled back"]
fn real_literature_source_smoke() {
    let file = PathBuf::from(
        std::env::var_os("SUNA_LIVE_LITERATURE_FILE")
            .expect("provide downloaded real literature file"),
    );
    let query = std::env::var("SUNA_LIVE_LITERATURE_QUERY").unwrap_or_else(|_| "frictional".into());
    let root = std::env::temp_dir().join(format!("suna-literature-live-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let (connection, _) = crate::storage::database::open(&root.join("suna.sqlite3")).unwrap();
    drop(connection);
    let source = ingest_file(&file, &root, IngestLimits::default()).unwrap();
    let parsed =
        parse_document(&source.stored_path, source.format, ParseLimits::default()).unwrap();
    let chunks = chunk_document(&parsed, &ChunkPolicy::default()).unwrap();
    assert!(chunks.len() > 1, "real paper must produce multiple chunks");
    tauri::async_runtime::block_on(async {
        let pool = configured_pool().await;
        let schema = fixture_schema(&pool).await;
        let kb = Uuid::new_v4();
        let document = Uuid::new_v4();
        let version = Uuid::new_v4();
        sqlx::query("INSERT INTO knowledge_bases(id,name) VALUES($1,'Real OA literature')")
            .bind(kb)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_documents(id,knowledge_base_id,display_name,source_kind,source_path,content_sha256) VALUES($1,$2,$3,'paper',$4,$5)")
            .bind(document).bind(kb).bind(file.file_name().unwrap().to_string_lossy().as_ref())
            .bind(source.stored_path.to_string_lossy().as_ref()).bind(&source.content_sha256).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO document_versions(id,document_id,content_sha256,mime_type,parser,parser_version,chunk_policy_version,activated_at,manifest_sealed) VALUES($1,$2,$3,$4,'builtin','1','steel-v1',now(),TRUE)")
            .bind(version).bind(document).bind(&source.content_sha256).bind(&source.mime_type).execute(&pool).await.unwrap();
        for chunk in &chunks {
            sqlx::query("INSERT INTO document_chunks(id,version_id,ordinal,text,source_location) VALUES($1,$2,$3,$4,$5)")
                .bind(Uuid::new_v4()).bind(version).bind(chunk.ordinal as i32).bind(&chunk.text)
                .bind(serde_json::to_value(&chunk.source_location).unwrap()).execute(&pool).await.unwrap();
        }
        let mut gateway = DesktopSteelAgentGateway::new(root.join("suna.sqlite3"), "local");
        gateway.postgres_pool = Some(pool.clone());
        let search = gateway
            .search_literature(json!({"query":query,"knowledge_base_ids":[kb]}))
            .await
            .unwrap();
        assert_eq!(search["mode"], "postgresql_fts");
        assert!(!search["results"].as_array().unwrap().is_empty());
        let read = gateway
            .read_literature_section(
                json!({"query":query,"knowledge_base_ids":[kb],"mode":"section"}),
            )
            .await
            .unwrap();
        assert_eq!(read["success"], true);
        assert!(read["content"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains(&query.to_lowercase()));
        assert!(read["content"].as_str().unwrap().chars().count() > 100);
        sqlx::raw_sql("ROLLBACK").execute(&pool).await.unwrap();
        let remains: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname=$1)")
                .bind(schema)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!remains);
        pool.close().await;
        println!("Real literature: {} bytes, {} parsed chunks; production PostgreSQL search and section reading verified; transaction rolled back", source.byte_len, chunks.len());
    });
    std::fs::remove_dir_all(root).unwrap();
}
