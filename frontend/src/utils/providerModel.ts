import type { ProviderProfileResponse } from "../bridge/desktop";

export function isChatProfile(profile: Pick<ProviderProfileResponse, "kind" | "model_id">): boolean {
  return profile.kind !== "mineru"
    && Boolean(profile.model_id?.trim())
    && !/embedding|rerank|bge-m3/i.test(profile.model_id ?? "");
}
