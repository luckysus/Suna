use crate::app::provider_commands::logic::probe_provider;
use crate::providers::capabilities::{
    ChatEvent, ChatImage, ChatMessage, ChatProvider, ChatRequest, EmbeddingProvider,
    RerankDocument, RerankProvider,
};
use crate::providers::profiles::{ProviderCapability, ProviderKind, ProviderProfileRecord};
use crate::providers::{
    configured_chat_provider, configured_embedding_provider, configured_rerank_provider,
    SiliconFlowPlan,
};
use crate::storage::secrets::SecretValue;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Deserialize)]
pub struct ProviderDebugInput {
    pub profile_id: String,
    pub input: Option<String>,
    pub documents: Vec<String>,
    pub options: Option<ProviderDebugOptions>,
    pub file_name: Option<String>,
    pub file_base64: Option<String>,
    pub file_mime: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ProviderDebugOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub similarity: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ProviderDebugResult {
    pub ok: bool,
    pub elapsed_ms: u64,
    pub request: serde_json::Value,
    pub raw_response: serde_json::Value,
    pub observations: serde_json::Value,
    pub error: Option<String>,
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> Option<f32> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let dot = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>();
    let norm_a = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return None;
    }
    Some(dot / (norm_a * norm_b))
}

async fn debug_asr(
    record: &ProviderProfileRecord,
    credential: Option<SecretValue>,
    file_base64: &str,
    file_mime: Option<&str>,
    file_name: Option<&str>,
) -> Result<serde_json::Value, String> {
    let api_key = credential
        .ok_or_else(|| "ASR 需要配置 API Key".to_string())?
        .expose()
        .to_string();
    let base = record.profile.base_url.trim_end_matches('/');
    let url = format!("{base}/audio/transcriptions");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(file_base64.trim())
        .map_err(|error| format!("invalid audio base64: {error}"))?;
    let mime = file_mime
        .filter(|value| !value.is_empty())
        .unwrap_or("audio/mpeg");
    let name = file_name
        .filter(|value| !value.is_empty())
        .unwrap_or("audio.mp3");
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(name.to_string())
        .mime_str(mime)
        .map_err(|error| error.to_string())?;
    let form = reqwest::multipart::Form::new()
        .text(
            "model",
            record
                .profile
                .model_id
                .clone()
                .unwrap_or_else(|| "whisper-1".to_string()),
        )
        .part("file", part);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .post(url)
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("ASR HTTP {}: {}", status, body));
    }
    let value: serde_json::Value =
        serde_json::from_str(&body).unwrap_or(serde_json::json!({ "text": body }));
    Ok(value)
}

pub async fn debug_provider(
    record: ProviderProfileRecord,
    payload: ProviderDebugInput,
    credential: Option<SecretValue>,
) -> ProviderDebugResult {
    let started = Instant::now();
    let capability = match record.model_type.as_str() {
        "embedding" => ProviderCapability::Embedding,
        "rerank" => ProviderCapability::Rerank,
        "document_parser" => ProviderCapability::DocumentParser,
        "vllm" => ProviderCapability::Vllm,
        "asr" => ProviderCapability::Asr,
        _ => ProviderCapability::Chat,
    };
    let request_preview = serde_json::json!({
        "profile_id": record.profile.id,
        "model_type": record.model_type,
        "model_id": record.profile.model_id,
        "input": payload.input,
        "documents": payload.documents,
        "options": payload.options,
        "file": payload.file_name,
        "file_mime": payload.file_mime,
    });

    if record.model_type == "asr" {
        let file = payload.file_base64.as_deref().unwrap_or("");
        if file.is_empty() {
            return finish(
                started,
                request_preview,
                serde_json::Value::Null,
                serde_json::json!({}),
                false,
                Some("ASR 调试需要上传音频文件".to_string()),
            );
        }
        match debug_asr(
            &record,
            credential,
            file,
            payload.file_mime.as_deref(),
            payload.file_name.as_deref(),
        )
        .await
        {
            Ok(raw) => {
                let transcript_chars = raw
                    .get("text")
                    .and_then(|value| value.as_str())
                    .map(|value| value.len())
                    .unwrap_or(0);
                return finish(
                    started,
                    request_preview,
                    raw,
                    serde_json::json!({ "transcript_chars": transcript_chars }),
                    true,
                    None,
                );
            }
            Err(error) => {
                return finish(
                    started,
                    request_preview,
                    serde_json::Value::Null,
                    serde_json::json!({}),
                    false,
                    Some(error),
                );
            }
        }
    }

    if record.model_type == "vllm" {
        let file = payload.file_base64.as_deref().unwrap_or("");
        if file.is_empty() {
            return finish(
                started,
                request_preview,
                serde_json::Value::Null,
                serde_json::json!({}),
                false,
                Some("视觉模型调试需要上传图片".to_string()),
            );
        }
        let mime = payload
            .file_mime
            .as_deref()
            .filter(|value| !value.is_empty())
            .unwrap_or("image/png");
        let prompt = payload
            .input
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("请描述这张图片中的内容。");
        let provider = match configured_chat_provider(record.profile.clone(), credential) {
            Ok(provider) => provider,
            Err(error) => {
                return finish(
                    started,
                    request_preview,
                    serde_json::Value::Null,
                    serde_json::json!({}),
                    false,
                    Some(error.to_string()),
                );
            }
        };
        let image = ChatImage {
            data: file.to_string(),
            mime: mime.to_string(),
        };
        let request = ChatRequest {
            messages: vec![
                ChatMessage::new("system", "You are a helpful vision assistant."),
                ChatMessage::with_images("user", prompt, vec![image]),
            ],
            temperature: payload
                .options
                .as_ref()
                .and_then(|value| value.temperature)
                .unwrap_or(0.2),
            tools: None,
            response_format: None,
            reasoning_effort: None,
            max_tokens: payload
                .options
                .and_then(|value| value.max_tokens)
                .map(|value| value as usize),
            stop: None,
        };
        let mut answer = String::new();
        let chat_result = provider
            .chat(
                request,
                &mut |event| {
                    if let ChatEvent::TextDelta(delta) = event {
                        answer.push_str(&delta);
                    }
                },
                &|| false,
            )
            .await;
        return match chat_result {
            Ok(response) => finish(
                started,
                request_preview,
                serde_json::json!({
                    "text": response.text,
                    "answer_preview": answer,
                }),
                serde_json::json!({ "vision": true, "image_mime": mime }),
                true,
                None,
            ),
            Err(error) => finish(
                started,
                request_preview,
                serde_json::Value::Null,
                serde_json::json!({}),
                false,
                Some(error.to_string()),
            ),
        };
    }

    if record.model_type == "embedding" {
        let similarity = payload
            .options
            .as_ref()
            .and_then(|value| value.similarity)
            .unwrap_or(false);
        let mut inputs = Vec::new();
        if similarity {
            if !payload.documents.is_empty() {
                inputs.extend(payload.documents.iter().take(2).cloned());
            } else {
                let lines = payload
                    .input
                    .as_deref()
                    .unwrap_or("")
                    .lines()
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .take(2)
                    .collect::<Vec<_>>();
                inputs.extend(lines.into_iter().map(str::to_string));
            }
            if inputs.len() < 2 {
                return finish(
                    started,
                    request_preview,
                    serde_json::Value::Null,
                    serde_json::json!({}),
                    false,
                    Some("相似度模式需要两段文本（换行分隔或文档列表前两条）".to_string()),
                );
            }
        } else {
            let text = payload
                .input
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Suna embedding debug");
            inputs.push(text.to_string());
        }
        let model = record.profile.model_id.clone();
        let provider = match configured_embedding_provider(
            record.profile.clone(),
            credential,
            SiliconFlowPlan::Free,
            model,
        ) {
            Ok(provider) => provider,
            Err(error) => {
                return finish(
                    started,
                    request_preview,
                    serde_json::Value::Null,
                    serde_json::json!({}),
                    false,
                    Some(error.to_string()),
                );
            }
        };
        let input_count = inputs.len();
        match provider.embed(inputs).await {
            Ok(response) => {
                let sim = if similarity && response.vectors.len() >= 2 {
                    cosine_similarity(&response.vectors[0], &response.vectors[1])
                } else {
                    None
                };
                return finish(
                    started,
                    request_preview,
                    serde_json::to_value(&response).unwrap_or(serde_json::Value::Null),
                    serde_json::json!({
                        "vector_count": response.vectors.len(),
                        "dimension": response.vectors.first().map(|vector| vector.len()),
                        "cosine_similarity": sim,
                    }),
                    true,
                    None,
                );
            }
            Err(error) => {
                return finish(
                    started,
                    request_preview,
                    serde_json::Value::Null,
                    serde_json::json!({ "input_count": input_count }),
                    false,
                    Some(error.to_string()),
                );
            }
        }
    }

    if record.model_type == "rerank" {
        let query = payload
            .input
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Suna rerank debug");
        let documents = if payload.documents.is_empty() {
            vec![
                RerankDocument {
                    id: "doc-1".to_string(),
                    text: "第一段候选文档".to_string(),
                },
                RerankDocument {
                    id: "doc-2".to_string(),
                    text: query.to_string(),
                },
            ]
        } else {
            payload
                .documents
                .iter()
                .enumerate()
                .map(|(index, text)| RerankDocument {
                    id: format!("doc-{index}"),
                    text: text.clone(),
                })
                .collect()
        };
        let model = record.profile.model_id.clone();
        let provider = match configured_rerank_provider(
            record.profile.clone(),
            credential,
            SiliconFlowPlan::Free,
            model,
        ) {
            Ok(provider) => provider,
            Err(error) => {
                return finish(
                    started,
                    request_preview,
                    serde_json::Value::Null,
                    serde_json::json!({}),
                    false,
                    Some(error.to_string()),
                );
            }
        };
        match provider.rerank(query.to_string(), documents).await {
            Ok(results) => {
                return finish(
                    started,
                    request_preview,
                    serde_json::to_value(&results).unwrap_or(serde_json::Value::Null),
                    serde_json::json!({
                        "query_chars": query.len(),
                        "result_count": results.len(),
                    }),
                    true,
                    None,
                );
            }
            Err(error) => {
                return finish(
                    started,
                    request_preview,
                    serde_json::Value::Null,
                    serde_json::json!({}),
                    false,
                    Some(error.to_string()),
                );
            }
        }
    }

    if record.profile.kind == ProviderKind::MinerU {
        let probe = probe_provider(record.profile.clone(), credential, Some(capability)).await;
        return finish(
            started,
            request_preview,
            serde_json::to_value(&probe).unwrap_or(serde_json::Value::Null),
            serde_json::json!({ "capability": "document_parser" }),
            probe.ok,
            probe.error_code,
        );
    }

    let user_message = payload
        .input
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("ping");
    let provider = match configured_chat_provider(record.profile.clone(), credential) {
        Ok(provider) => provider,
        Err(error) => {
            return finish(
                started,
                request_preview,
                serde_json::Value::Null,
                serde_json::json!({}),
                false,
                Some(error.to_string()),
            );
        }
    };
    let request = ChatRequest::single_turn("You are a helpful assistant.", user_message);
    let mut answer = String::new();
    let chat_result = provider
        .chat(
            request,
            &mut |event| {
                if let ChatEvent::TextDelta(delta) = event {
                    answer.push_str(&delta);
                }
            },
            &|| false,
        )
        .await;
    match chat_result {
        Ok(response) => finish(
            started,
            request_preview,
            serde_json::json!({
                "text": response.text,
                "answer_preview": answer,
                "cancelled": response.cancelled,
            }),
            serde_json::json!({
                "input_chars": user_message.len(),
                "has_file": payload.file_base64.is_some(),
            }),
            true,
            None,
        ),
        Err(error) => finish(
            started,
            request_preview,
            serde_json::Value::Null,
            serde_json::json!({ "input_chars": user_message.len() }),
            false,
            Some(error.to_string()),
        ),
    }
}

fn finish(
    started: Instant,
    request: serde_json::Value,
    raw_response: serde_json::Value,
    observations: serde_json::Value,
    ok: bool,
    error: Option<String>,
) -> ProviderDebugResult {
    ProviderDebugResult {
        ok,
        elapsed_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        request,
        raw_response,
        observations,
        error,
    }
}
