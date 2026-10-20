import type { ProviderProfileResponse, ProfileModelType } from "../bridge/desktop";

export function profileModelType(
  profile: Pick<ProviderProfileResponse, "model_type" | "kind" | "model_id">,
): ProfileModelType {
  if (profile.model_type) return profile.model_type;
  if (profile.kind === "mineru") return "document_parser";
  const modelId = (profile.model_id ?? "").toLowerCase();
  if (modelId.includes("whisper") || modelId.includes("paraformer")) return "asr";
  if (modelId.includes("-vl") || modelId.includes("vision")) return "vllm";
  if (modelId.includes("rerank")) return "rerank";
  if (modelId.includes("embedding") || modelId.includes("bge-m3")) return "embedding";
  return "chat";
}

export function isChatProfile(profile: Pick<ProviderProfileResponse, "model_type" | "kind" | "model_id">): boolean {
  return profileModelType(profile) === "chat" && profile.kind !== "mineru";
}

export function isVisionProfile(profile: Pick<ProviderProfileResponse, "model_type">): boolean {
  return profile.model_type === "vllm";
}
