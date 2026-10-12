pub mod anthropic;
pub mod capabilities;
pub mod http;
pub mod mineru;
pub mod ollama;
pub mod openai;
pub mod profiles;
pub mod siliconflow;

use self::anthropic::AnthropicProvider;
use self::capabilities::{
    ChatEvent, ChatProvider, ChatRequest, ChatResponse, EmbeddingProvider, EmbeddingResponse,
    ProviderCapabilities, RerankDocument, RerankProvider, RerankResult,
};
use self::http::{ProviderError, ProviderErrorCode};
use self::ollama::OllamaProvider;
use self::openai::OpenAiProvider;
use self::profiles::{ProviderCapability, ProviderKind, ProviderProfile};
use self::siliconflow::RetrievalProvider;
pub use self::siliconflow::SiliconFlowPlan;
use crate::storage::secrets::SecretValue;

pub enum ConfiguredEmbeddingProvider {
    SiliconFlow(RetrievalProvider),
    Qwen(RetrievalProvider),
}

pub enum ConfiguredRerankProvider {
    SiliconFlow(RetrievalProvider),
    Qwen(RetrievalProvider),
}

pub fn configured_embedding_provider(
    profile: ProviderProfile,
    credential: Option<SecretValue>,
    plan: SiliconFlowPlan,
    model: Option<String>,
) -> Result<ConfiguredEmbeddingProvider, ProviderError> {
    let qwen = profile.kind == ProviderKind::Qwen;
    RetrievalProvider::with_models(profile, credential, plan, model, None).map(|provider| {
        if qwen {
            ConfiguredEmbeddingProvider::Qwen(provider)
        } else {
            ConfiguredEmbeddingProvider::SiliconFlow(provider)
        }
    })
}

pub fn configured_rerank_provider(
    profile: ProviderProfile,
    credential: Option<SecretValue>,
    plan: SiliconFlowPlan,
    model: Option<String>,
) -> Result<ConfiguredRerankProvider, ProviderError> {
    let qwen = profile.kind == ProviderKind::Qwen;
    RetrievalProvider::with_models(profile, credential, plan, None, model).map(|provider| {
        if qwen {
            ConfiguredRerankProvider::Qwen(provider)
        } else {
            ConfiguredRerankProvider::SiliconFlow(provider)
        }
    })
}

pub async fn probe_retrieval(
    profile: ProviderProfile,
    credential: Option<SecretValue>,
    capability: ProviderCapability,
) -> Result<u16, ProviderError> {
    let model = profile.model_id.clone();
    let provider = match capability {
        ProviderCapability::Embedding => {
            RetrievalProvider::with_models(profile, credential, SiliconFlowPlan::Free, model, None)?
        }
        ProviderCapability::Rerank => {
            RetrievalProvider::with_models(profile, credential, SiliconFlowPlan::Free, None, model)?
        }
        _ => {
            return Err(ProviderError::new(
                ProviderErrorCode::UnsupportedCapability,
                None,
                "Retrieval probe requires embedding or rerank capability",
            ))
        }
    };
    match capability {
        ProviderCapability::Embedding => {
            provider
                .embed(vec!["Suna provider probe".to_string()])
                .await?;
        }
        ProviderCapability::Rerank => {
            provider
                .rerank(
                    "Suna provider probe".to_string(),
                    vec![RerankDocument {
                        id: "probe".to_string(),
                        text: "Suna provider probe".to_string(),
                    }],
                )
                .await?;
        }
        _ => unreachable!("unsupported retrieval probe capability"),
    }
    Ok(reqwest::StatusCode::OK.as_u16())
}

impl EmbeddingProvider for ConfiguredEmbeddingProvider {
    fn capabilities(&self) -> &ProviderCapabilities {
        match self {
            Self::SiliconFlow(provider) | Self::Qwen(provider) => {
                EmbeddingProvider::capabilities(provider)
            }
        }
    }

    async fn embed(&self, inputs: Vec<String>) -> Result<EmbeddingResponse, ProviderError> {
        match self {
            Self::SiliconFlow(provider) | Self::Qwen(provider) => provider.embed(inputs).await,
        }
    }
}

impl RerankProvider for ConfiguredRerankProvider {
    fn capabilities(&self) -> &ProviderCapabilities {
        match self {
            Self::SiliconFlow(provider) | Self::Qwen(provider) => {
                RerankProvider::capabilities(provider)
            }
        }
    }

    async fn rerank(
        &self,
        query: String,
        documents: Vec<RerankDocument>,
    ) -> Result<Vec<RerankResult>, ProviderError> {
        match self {
            Self::SiliconFlow(provider) | Self::Qwen(provider) => {
                provider.rerank(query, documents).await
            }
        }
    }
}

pub enum ConfiguredChatProvider {
    Anthropic(AnthropicProvider),
    OpenAi(OpenAiProvider),
    DeepSeek(OpenAiProvider),
    Ollama(OllamaProvider),
}

impl ConfiguredChatProvider {
    pub fn with_request_timeout(self, timeout: std::time::Duration) -> Result<Self, ProviderError> {
        match self {
            Self::Anthropic(provider) => {
                provider.with_request_timeout(timeout).map(Self::Anthropic)
            }
            Self::OpenAi(provider) => provider.with_request_timeout(timeout).map(Self::OpenAi),
            Self::DeepSeek(provider) => provider.with_request_timeout(timeout).map(Self::DeepSeek),
            Self::Ollama(provider) => provider.with_request_timeout(timeout).map(Self::Ollama),
        }
    }
}

pub fn configured_chat_provider(
    profile: ProviderProfile,
    credential: Option<SecretValue>,
) -> Result<ConfiguredChatProvider, ProviderError> {
    match profile.kind {
        ProviderKind::Anthropic => {
            AnthropicProvider::new(profile, credential).map(ConfiguredChatProvider::Anthropic)
        }
        ProviderKind::Ollama => OllamaProvider::new(profile).map(ConfiguredChatProvider::Ollama),
        ProviderKind::DeepSeek => {
            OpenAiProvider::new(profile, credential).map(ConfiguredChatProvider::DeepSeek)
        }
        _ if profile.kind.supports(ProviderCapability::Chat) => {
            OpenAiProvider::new(profile, credential).map(ConfiguredChatProvider::OpenAi)
        }
        _ => Err(ProviderError::new(
            ProviderErrorCode::UnsupportedCapability,
            None,
            "provider profile does not support chat",
        )),
    }
}

impl ChatProvider for ConfiguredChatProvider {
    fn capabilities(&self) -> &ProviderCapabilities {
        match self {
            Self::Anthropic(provider) => provider.capabilities(),
            Self::OpenAi(provider) => provider.capabilities(),
            Self::DeepSeek(provider) => provider.capabilities(),
            Self::Ollama(provider) => provider.capabilities(),
        }
    }

    async fn chat(
        &self,
        request: ChatRequest,
        on_event: &mut (dyn FnMut(ChatEvent) + Send),
        is_cancelled: &(dyn Fn() -> bool + Send + Sync),
    ) -> Result<ChatResponse, ProviderError> {
        match self {
            Self::Anthropic(provider) => provider.chat(request, on_event, is_cancelled).await,
            Self::OpenAi(provider) => provider.chat(request, on_event, is_cancelled).await,
            Self::DeepSeek(provider) => provider.chat(request, on_event, is_cancelled).await,
            Self::Ollama(provider) => provider.chat(request, on_event, is_cancelled).await,
        }
    }
}
