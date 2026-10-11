/**
 * 第 71 章：统一错误处理。
 *
 * 桥接层（Tauri 命令）的失败信息可能带低层路径、连接串、SQL 或 Rust backtrace。
 * 这些细节适合放进诊断页，**不能**直接出现在业务 UI 里，更不能泄漏凭据。
 *
 * 因此全应用统一走这里：
 * - `errorMessage(cause, fallback)`：脱敏后的短消息（拿不到安全信息时用 fallback）；
 * - `describeError(cause, fallback)`：进一步归类成「任务执行失败 / 模型连接失败 / 工具不可用 / 数据格式错误」，
 *   供需要展示「原因 + 是否可重试」的界面使用。
 */

export type ErrorCategory = "connection" | "tool" | "data" | "permission" | "timeout" | "unknown";

export type FriendlyError = {
  /** 面向用户的原因标题。 */
  title: string;
  /** 已脱敏的细节；没有安全细节时为空串。 */
  detail: string;
  category: ErrorCategory;
  /** 是否建议用户重试。 */
  retryable: boolean;
};

const CATEGORY_TITLES: Record<ErrorCategory, string> = {
  connection: "模型连接失败",
  tool: "工具不可用",
  data: "数据格式错误",
  permission: "权限不足",
  timeout: "任务超时",
  unknown: "任务执行失败",
};

const LOW_LEVEL_PATTERN = /(?:RUST_BACKTRACE|panicked at|backtrace|stack trace|\btarget[\\/]src[\\/]|(?:postgres(?:ql)?|mysql|mssql|sqlite):\/\/|(?:sqlstate|connection string|database password)|\b(?:select|insert|update|delete)\b[\s\S]+\b(?:from|into|set|where)\b)/i;
const CREDENTIAL_PATTERN = /(?:password|passwd|api[_ -]?key|access[_ -]?token|bearer\s+|secret|authorization)/i;

/** 去掉换行、遮蔽凭据、丢弃低层诊断；返回可安全展示的短消息（可能为空串）。 */
export function redactErrorMessage(cause: unknown): string {
  const raw = cause instanceof Error ? cause.message : typeof cause === "string" ? cause : "";
  if (!raw.trim()) return "";
  const normalized = raw
    .replace(/\r?\n[\s\S]*/g, "")
    .replace(/((?:password|passwd|api[_ -]?key|token|secret|authorization))\s*[=:]\s*[^,;\s]+/gi, "$1=[已隐藏]")
    .replace(/([a-z][a-z0-9+.-]*:\/\/[^:\s/@]+:)[^@\s]+@/gi, "$1[已隐藏]@")
    .trim();
  if (!normalized || LOW_LEVEL_PATTERN.test(normalized)) return "";
  // 即使没有 `key=value` 形式，也不要展示凭据类诊断。
  if (CREDENTIAL_PATTERN.test(normalized)) return "";
  return normalized.length > 220 ? `${normalized.slice(0, 217)}…` : normalized;
}

/** 第 71 章统一入口：脱敏短消息，拿不到安全信息时回落到调用方给的兜底文案。 */
export function errorMessage(cause: unknown, fallback: string): string {
  const safe = redactErrorMessage(cause);
  return safe || fallback;
}

function classify(raw: string): ErrorCategory {
  if (/(?:timeout|timed out|超时|ETIMEDOUT)/i.test(raw)) return "timeout";
  if (/(?:permission|forbidden|unauthor|denied|403|401|权限)/i.test(raw)) return "permission";
  if (/(?:econnrefused|enotfound|network|fetch failed|dns|连接|connect|provider|api key|模型)/i.test(raw)) return "connection";
  if (/(?:tool|mcp|command not found|spawn|工具)/i.test(raw)) return "tool";
  if (/(?:parse|invalid json|schema|format|类型|格式|解析)/i.test(raw)) return "data";
  return "unknown";
}

/** 归类错误，供需要展示「原因 + 重试」的界面使用。 */
export function describeError(cause: unknown, fallback = "任务执行失败，请重试"): FriendlyError {
  const safe = redactErrorMessage(cause);
  const raw = cause instanceof Error ? cause.message : typeof cause === "string" ? cause : "";
  const category = raw.trim() ? classify(raw) : "unknown";
  return {
    title: CATEGORY_TITLES[category],
    detail: safe || (category === "unknown" ? fallback : ""),
    category,
    retryable: category !== "permission" && category !== "data",
  };
}
