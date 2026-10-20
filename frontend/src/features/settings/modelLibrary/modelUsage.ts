import type { ModelUsageDetails } from "../../../bridge/desktop";

export const MODEL_IN_USE_PREFIX = "MODEL_IN_USE:";
export const MODEL_IN_USE_CODE = 2300;

export const USAGE_BINDING_LABELS: Record<string, string> = {
  chat_model: "对话默认",
  embedding_model: "向量嵌入默认",
  rerank_model: "重排默认",
  document_parser_model: "文档解析默认",
  vlm_model: "视觉模型默认",
  asr_model: "语音识别默认",
  summary_model: "摘要模型",
  image_processing_model: "图像处理",
  wiki_synthesis_model: "Wiki 合成",
  auto_tag_model: "自动标签",
  query_understand_model: "查询理解",
  follow_up_model: "追问模型",
  extract_model: "抽取模型",
  unknown: "其他引用",
};

export function parseModelUsageFromError(error: unknown): ModelUsageDetails | null {
  const message =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : typeof error === "object" && error && "message" in error && typeof (error as { message: unknown }).message === "string"
          ? (error as { message: string }).message
          : "";
  if (!message.startsWith(MODEL_IN_USE_PREFIX)) return null;
  try {
    const payload = JSON.parse(message.slice(MODEL_IN_USE_PREFIX.length)) as {
      code?: number;
      details?: ModelUsageDetails;
    };
    if (payload.code !== MODEL_IN_USE_CODE || !payload.details) return null;
    return payload.details;
  } catch {
    return null;
  }
}

export function usageBindingLabel(binding: string): string {
  return USAGE_BINDING_LABELS[binding] ?? USAGE_BINDING_LABELS.unknown;
}
