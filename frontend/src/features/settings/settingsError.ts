import { errorMessage } from "../../utils/errorMessage";

/**
 * 设置页专用的错误文案入口。
 *
 * 第 71 章已把脱敏逻辑提升为全应用共享的 `utils/errorMessage`，
 * 这里保留原函数名以免大范围改动调用方与测试。
 */
export function settingsErrorMessage(cause: unknown, fallback: string): string {
  return errorMessage(cause, fallback);
}
