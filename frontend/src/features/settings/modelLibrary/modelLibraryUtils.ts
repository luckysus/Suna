import type { ProfileModelType, ProviderProfileResponse } from "../../../bridge/desktop";
import type { ProviderCapability } from "../../../bridge/desktop";
import { isChatProfile } from "../../../utils/providerModel";

export type ModelLibraryFilter = "all" | ProfileModelType;

export const MODEL_TYPE_LABELS: Record<ProfileModelType, string> = {
  chat: "对话",
  embedding: "向量嵌入",
  rerank: "语义重排",
  document_parser: "文档解析",
  vllm: "视觉",
  asr: "语音",
};

export function inferProfileModelType(profile: ProviderProfileResponse): ProfileModelType {
  if (profile.model_type) return profile.model_type;
  if (profile.kind === "mineru") return "document_parser";
  const modelId = (profile.model_id ?? "").toLowerCase();
  if (modelId.includes("whisper") || modelId.includes("paraformer") || modelId.includes("asr")) {
    return "asr";
  }
  if (modelId.includes("vl-") || modelId.includes("-vl") || modelId.includes("vision") || modelId.includes("llava")) {
    return "vllm";
  }
  if (modelId.includes("rerank")) return "rerank";
  if (modelId.includes("embedding") || modelId.includes("bge-m3") || modelId.includes("embed")) {
    return "embedding";
  }
  return "chat";
}

export function capabilityForModelType(type: ProfileModelType): ProviderCapability {
  if (type === "embedding") return "embedding";
  if (type === "rerank") return "rerank";
  if (type === "document_parser") return "document_parser";
  if (type === "vllm") return "vllm";
  if (type === "asr") return "asr";
  return "chat";
}

export function slotForModelType(type: ProfileModelType): "chat" | "embedding" | "reranker" | "mineru" {
  if (type === "document_parser") return "mineru";
  if (type === "rerank") return "reranker";
  if (type === "embedding") return "embedding";
  if (type === "vllm" || type === "asr") return "chat";
  return "chat";
}

export function matchesModelQuery(profile: ProviderProfileResponse, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const haystack = [
    profile.display_name,
    profile.model_id ?? "",
    profile.base_url,
    profile.kind,
    MODEL_TYPE_LABELS[inferProfileModelType(profile)],
  ]
    .join(" ")
    .toLowerCase();
  return haystack.includes(q);
}

export function filterChatProfiles(profiles: ProviderProfileResponse[]): ProviderProfileResponse[] {
  return profiles.filter((profile) => profile.enabled && isChatProfile(profile));
}

export function copyDisplayName(name: string): string {
  const suffix = " (副本)";
  if (name.endsWith(suffix)) return `${name.slice(0, -suffix.length)}${suffix} 2`;
  return `${name}${suffix}`;
}
