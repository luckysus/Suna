//! Opt-in smoke test for the real chat, PostgreSQL/pgvector, embedding, and
//! reranker integrations. CI keeps this test compiled but skips the network
//! calls unless explicitly selected with `--ignored`.

use std::env;
use std::time::Duration;

use suna::providers::capabilities::{
    ChatMessage, ChatProvider, ChatRequest, EmbeddingProvider, RerankDocument, RerankProvider,
};
use suna::providers::profiles::{resolve_chat_profile, ProviderKind, ProviderProfile};
use suna::providers::{
    configured_chat_provider, configured_embedding_provider, configured_rerank_provider,
    SiliconFlowPlan,
};
use suna::storage::secrets::SecretValue;

fn required(name: &str) -> String {
    env::var(name).unwrap_or_else(|_| panic!("live smoke requires environment variable {name}"))
}

fn optional_secret(name: &str) -> Option<SecretValue> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(|value| SecretValue::new(value).expect("live smoke secret is valid"))
}

fn siliconflow_profile(base_url: String, model_id: String) -> ProviderProfile {
    ProviderProfile {
        id: uuid::Uuid::new_v4(),
        kind: ProviderKind::SiliconFlow,
        display_name: "Suna live smoke".to_string(),
        base_url,
        model_id: Some(model_id),
        secret_ref: None,
        enabled: true,
    }
}

#[test]
#[ignore = "requires explicit real provider credentials and PostgreSQL configuration"]
fn real_stack_smoke() {
    let chat_kind = required("SUNA_LIVE_CHAT_PROVIDER");
    let chat_base_url = required("SUNA_LIVE_CHAT_BASE_URL");
    let chat_model = required("SUNA_LIVE_CHAT_MODEL");
    let chat_profile = resolve_chat_profile(&chat_kind, &chat_base_url, &chat_model)
        .expect("live chat provider profile is valid");
    let chat_credential = optional_secret("SUNA_LIVE_CHAT_API_KEY");
    let chat = configured_chat_provider(chat_profile, chat_credential)
        .expect("live chat provider can be constructed");
    let mut events = Vec::new();
    let answer = tauri::async_runtime::block_on(chat.chat(
        ChatRequest {
            messages: vec![
                ChatMessage::new("system", "Reply with a short health check."),
                ChatMessage::new("user", "Reply exactly: SUNA_LIVE_OK"),
            ],
            temperature: 0.0,
            tools: None,
            response_format: None,
            reasoning_effort: None,
            max_tokens: Some(32),
            stop: None,
        },
        &mut |event| events.push(event),
        &|| false,
    ))
    .expect("live chat request succeeds");
    assert!(
        !answer.text.trim().is_empty(),
        "live chat returned an empty answer"
    );
    assert!(!events.is_empty(), "live chat returned no stream events");

    let postgres_url = required("SUNA_LIVE_POSTGRES_URL");
    let pool = tauri::async_runtime::block_on(
        sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_secs(10))
            .max_connections(2)
            .connect(&postgres_url),
    )
    .expect("live PostgreSQL connection succeeds");
    let server_version: i64 = tauri::async_runtime::block_on(
        sqlx::query_scalar("SELECT current_setting('server_version_num')::bigint").fetch_one(&pool),
    )
    .expect("PostgreSQL version query succeeds");
    assert!(
        server_version >= 140_000,
        "PostgreSQL 14 or newer is required"
    );
    let vector_enabled: bool = tauri::async_runtime::block_on(
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector')")
            .fetch_one(&pool),
    )
    .expect("pgvector extension query succeeds");
    assert!(
        vector_enabled,
        "PostgreSQL pgvector extension is not enabled"
    );
    let knowledge_schema_ready: bool = tauri::async_runtime::block_on(
        sqlx::query_scalar("SELECT to_regclass('public.knowledge_bases') IS NOT NULL")
            .fetch_one(&pool),
    )
    .expect("knowledge schema query succeeds");
    assert!(
        knowledge_schema_ready,
        "Suna knowledge migrations are not installed"
    );
    tauri::async_runtime::block_on(pool.close());

    let siliconflow_base_url = required("SUNA_LIVE_SILICONFLOW_BASE_URL");
    let siliconflow_key = SecretValue::new(required("SUNA_LIVE_SILICONFLOW_API_KEY"))
        .expect("live SiliconFlow credential is valid");
    let embedding_model = required("SUNA_LIVE_EMBEDDING_MODEL");
    let embedding = configured_embedding_provider(
        siliconflow_profile(siliconflow_base_url.clone(), embedding_model),
        Some(siliconflow_key.clone()),
        SiliconFlowPlan::Free,
        Some(env::var("SUNA_LIVE_EMBEDDING_MODEL").expect("embedding model")),
    )
    .expect("live embedding provider can be constructed");
    let embedding_result = tauri::async_runtime::block_on(
        embedding.embed(vec!["Suna pgvector embedding smoke test".to_string()]),
    )
    .expect("live embedding request succeeds");
    assert_eq!(embedding_result.vectors.len(), 1);
    assert!(!embedding_result.vectors[0].is_empty());

    let reranker = configured_rerank_provider(
        siliconflow_profile(siliconflow_base_url, required("SUNA_LIVE_RERANK_MODEL")),
        Some(siliconflow_key),
        SiliconFlowPlan::Free,
        Some(required("SUNA_LIVE_RERANK_MODEL")),
    )
    .expect("live reranker provider can be constructed");
    let rerank_result = tauri::async_runtime::block_on(reranker.rerank(
        "steel yield strength".to_string(),
        vec![RerankDocument {
            id: "smoke".to_string(),
            text: "Steel yield strength is specified by grade and thickness.".to_string(),
        }],
    ))
    .expect("live reranker request succeeds");
    assert_eq!(rerank_result.len(), 1);
}

#[test]
#[ignore = "calls the real Bailian API using local environment credentials"]
fn real_bailian_chat_smoke() {
    let model = env::var("SUNA_LIVE_CHAT_MODEL").unwrap_or_else(|_| "qwen3.8-flash".into());
    let profile = resolve_chat_profile("qwen", &required("DASHSCOPE_BASE_URL"), &model).unwrap();
    let chat = configured_chat_provider(profile, optional_secret("DASHSCOPE_API_KEY")).unwrap();
    let mut events = Vec::new();
    let response = tauri::async_runtime::block_on(chat.chat(
        ChatRequest {
            messages: vec![ChatMessage::new(
                "user",
                "Reply with the single token SUNA_LIVE_OK.",
            )],
            temperature: 0.0,
            tools: None,
            response_format: None,
            reasoning_effort: None,
            max_tokens: Some(64),
            stop: None,
        },
        &mut |event| events.push(event),
        &|| false,
    ))
    .expect("real Bailian request succeeds");
    assert!(
        response.text.contains("SUNA_LIVE_OK"),
        "unexpected health-check response"
    );
    assert!(!events.is_empty(), "no real streaming events received");
    println!("Bailian {model}: streaming response and final token verified");
}

#[test]
#[ignore = "calls real Bailian Embedding/Rerank using local environment credentials"]
fn real_bailian_retrieval_smoke() {
    let base_url = required("DASHSCOPE_BASE_URL");
    let credential = optional_secret("DASHSCOPE_API_KEY");
    let embedding_model = env::var("SUNA_LIVE_EMBEDDING_MODEL")
        .unwrap_or_else(|_| suna::providers::siliconflow::QWEN_EMBEDDING_MODEL.into());
    let rerank_model = env::var("SUNA_LIVE_RERANK_MODEL")
        .unwrap_or_else(|_| suna::providers::siliconflow::QWEN_RERANK_MODEL.into());
    let profile = |model: &str| ProviderProfile {
        id: uuid::Uuid::new_v4(),
        kind: ProviderKind::Qwen,
        display_name: "Bailian live retrieval".into(),
        base_url: base_url.clone(),
        model_id: Some(model.into()),
        secret_ref: None,
        enabled: true,
    };
    let query = "Q355B钢材的屈服强度是多少？";
    let documents = vec![
        RerankDocument {
            id: "unrelated".into(),
            text: "海水温度为20摄氏度，主要受到太阳辐射和洋流影响。".into(),
        },
        RerankDocument {
            id: "steel".into(),
            text: "Q355B是低合金高强度结构钢，其屈服强度不小于355 MPa。".into(),
        },
        RerankDocument {
            id: "cooking".into(),
            text: "烹饪米饭时，先清洗大米，再加入适量清水。".into(),
        },
    ];
    let embedding = configured_embedding_provider(
        profile(&embedding_model),
        credential.clone(),
        SiliconFlowPlan::Free,
        Some(embedding_model.clone()),
    )
    .expect("construct production embedding provider");
    let inputs = std::iter::once(query.to_string())
        .chain(documents.iter().map(|d| d.text.clone()))
        .collect();
    let vectors = tauri::async_runtime::block_on(embedding.embed(inputs))
        .expect("real embedding request succeeds")
        .vectors;
    assert_eq!(vectors.len(), documents.len() + 1);
    let dimensions = vectors[0].len();
    assert!(dimensions > 0);
    for vector in &vectors {
        assert_eq!(vector.len(), dimensions);
        assert!(vector.iter().all(|v| v.is_finite()));
        assert!(vector.iter().any(|v| *v != 0.0));
    }
    let cosine = |a: &[f32], b: &[f32]| {
        let dot: f64 = a
            .iter()
            .zip(b)
            .map(|(a, b)| f64::from(*a) * f64::from(*b))
            .sum();
        let norm = |v: &[f32]| v.iter().map(|n| f64::from(*n).powi(2)).sum::<f64>().sqrt();
        dot / (norm(a) * norm(b))
    };
    let relevant = cosine(&vectors[0], &vectors[2]);
    let unrelated = cosine(&vectors[0], &vectors[1]);
    assert!(
        relevant > unrelated,
        "embedding must distinguish related steel text"
    );
    let reranker = configured_rerank_provider(
        profile(&rerank_model),
        credential,
        SiliconFlowPlan::Free,
        Some(rerank_model.clone()),
    )
    .expect("construct production rerank provider");
    let results = tauri::async_runtime::block_on(reranker.rerank(query.into(), documents.clone()))
        .expect("real rerank request succeeds");
    assert_eq!(results.len(), documents.len());
    assert!(results.iter().all(|r| r.score.is_finite()));
    let mut ids: Vec<_> = results.iter().map(|r| r.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["cooking", "steel", "unrelated"]);
    let top = results
        .iter()
        .max_by(|a, b| a.score.total_cmp(&b.score))
        .unwrap();
    assert_eq!(top.id, "steel");
    println!("Bailian {embedding_model}: {} vectors, {dimensions} dimensions, cosine steel={relevant:.4} unrelated={unrelated:.4}; {rerank_model}: {} finite scores, steel ranked first", vectors.len(), results.len());
}
