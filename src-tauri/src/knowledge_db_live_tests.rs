use super::*;
use crate::app::knowledge_commands::logic::{
    query_postgres_knowledge_with_pool, LocalKnowledgeQueryRequest,
};
use crate::storage::secrets::{KeyringSecretStore, SecretStore};

pub(crate) async fn configured_pool() -> PgPool {
    let root = std::env::var_os("SUNA_LIVE_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| dirs::data_dir().unwrap().join("com.suna.desktop"));
    let sqlite = rusqlite::Connection::open_with_flags(
        root.join("suna.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let config = settings::get(&sqlite, current_workspace_id(), CONFIG_KEY)
        .unwrap()
        .expect("configure PostgreSQL in Suna first");
    let config: KnowledgeDatabaseConfig = serde_json::from_str(&config).unwrap();
    let password = KeyringSecretStore
        .get(&secret_ref().unwrap())
        .expect("configured PostgreSQL credential");
    let initial_pool = connect(&config, password.expose()).await.unwrap();
    let options = initial_pool.connect_options().as_ref().clone();
    initial_pool.close().await;
    PgPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .unwrap()
}

pub(crate) async fn fixture_schema(pool: &PgPool) -> String {
    sqlx::raw_sql("BEGIN").execute(pool).await.unwrap();
    let schema = format!("suna_smoke_{}", Uuid::new_v4().simple());
    sqlx::raw_sql(&format!(
        "CREATE SCHEMA {schema}; SET LOCAL search_path = {schema}, public"
    ))
    .execute(pool)
    .await
    .unwrap();
    for migration in sqlx::migrate!("migrations/knowledge").iter() {
        sqlx::raw_sql(&migration.sql).execute(pool).await.unwrap();
    }
    schema
}

#[test]
#[ignore = "uses configured local PostgreSQL/pgvector; all fixture writes are rolled back"]
fn real_postgres_retrieval_sql() {
    tauri::async_runtime::block_on(async {
        let pool = configured_pool().await;
        assert!(check_vector(&pool).await.unwrap());
        // A single connection keeps both fixtures and production queries in this transaction.
        let schema = fixture_schema(&pool).await;
        let kb = Uuid::new_v4();
        let document = Uuid::new_v4();
        let version = Uuid::new_v4();
        let chunk = Uuid::new_v4();
        sqlx::query("INSERT INTO knowledge_bases(id,name) VALUES($1,'Suna smoke')")
            .bind(kb)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO source_documents(id,knowledge_base_id,display_name,source_kind,source_path,content_sha256,metadata) VALUES($1,$2,'Real SQL fixture','paper','smoke.txt','smoke',$3)")
            .bind(document).bind(kb).bind(serde_json::json!({"source":"Nature", "year":2026,"material":"steel","tags":["strength"]})).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO document_versions(id,document_id,content_sha256,mime_type,parser,parser_version,chunk_policy_version,activated_at,embedding_model_id,embedding_dimension,manifest_sealed) VALUES($1,$2,'smoke','text/plain','text','1','1',now(),'smoke-model',3,TRUE)")
            .bind(version).bind(document).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO document_chunks(id,version_id,ordinal,text,source_location) VALUES($1,$2,0,'steel yield strength',$3)")
            .bind(chunk).bind(version).bind(serde_json::json!({"kind":"text_offsets", "start":0,"end":20})).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO chunk_embeddings(chunk_id,knowledge_base_id,document_id,model_id,dimension,embedding) VALUES($1,$2,$3,'smoke-model',3,'[1,0,0]'::vector)")
            .bind(chunk).bind(kb).bind(document).execute(&pool).await.unwrap();
        let lexical_request = |source: &str| PostgresKnowledgeSearchRequest {
            knowledge_base_id: kb,
            query: "steel".into(),
            limit: 10,
            filters: Some(PostgresKnowledgeSearchFilters {
                source: Some(source.into()),
                ..Default::default()
            }),
        };
        let hits = search_postgres_lexical_pool(&pool, lexical_request("Nature"))
            .await
            .unwrap();
        assert_eq!(hits[0].chunk_id, chunk);
        assert!(
            search_postgres_lexical_pool(&pool, lexical_request("Other Journal"))
                .await
                .unwrap()
                .is_empty()
        );
        let hits = search_postgres_vectors_pool(
            &pool,
            PostgresVectorSearchRequest {
                knowledge_base_id: kb,
                embedding: vec![1., 0., 0.],
                limit: 10,
            },
        )
        .await
        .unwrap();
        assert_eq!(hits[0].chunk_id, chunk);
        let hits = search_postgres_hybrid_unfiltered_pool(
            &pool,
            PostgresHybridSearchRequest {
                knowledge_base_id: kb,
                query: "steel".into(),
                embedding: vec![1., 0., 0.],
                limit: 10,
                rrf_k: 60,
            },
        )
        .await
        .unwrap();
        assert_eq!(hits[0].chunk_id, chunk);
        for (source, threshold, expected) in [
            ("Nature", Some(0.8), 1),
            ("Other Journal", Some(0.8), 0),
            ("Nature", Some(1.1), 1),
        ] {
            let filters = PostgresKnowledgeSearchFilters {
                source: Some(source.into()),
                ..Default::default()
            };
            let hits = search_postgres_hybrid_pool(
                &pool,
                kb,
                "steel",
                &[1., 0., 0.],
                10,
                60,
                threshold,
                Some(&filters),
            )
            .await
            .unwrap();
            assert_eq!(hits.len(), expected);
        }
        for (source, expected) in [("Nature", 1), ("Other Journal", 0)] {
            let request: LocalKnowledgeQueryRequest = serde_json::from_value(serde_json::json!({"query":"steel","knowledge_base_ids":[kb],"dense_limit":0,"filters":{"source":source}})).unwrap();
            let pack = query_postgres_knowledge_with_pool(pool.clone(), request)
                .await
                .unwrap();
            assert_eq!(pack.evidence.len(), expected);
        }
        sqlx::raw_sql("ROLLBACK").execute(&pool).await.unwrap();
        let remains: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname=$1)")
                .bind(schema)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(!remains, "fixture schema must not persist");
        pool.close().await;
        println!("PostgreSQL/pgvector: lexical, vector, both hybrid paths, source filters and lexical evidence query verified; fixture transaction rolled back");
    });
}
