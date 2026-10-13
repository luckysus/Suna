/**
 * 第 76 章：跨层共享的导航类型。
 *
 * 放在 `types/` 而不是 `app/`，是为了让 `components/` 这类通用 UI
 * 不必反向依赖 `app/` 层。
 */

export type SectionId =
  | "projects"
  | "chat"
  | "knowledge"
  | "literature"
  | "data"
  | "prediction"
  | "optimization"
  | "experiment"
  | "agents"
  | "tools"
  | "mcp"
  | "skills"
  | "models"
  | "reports"
  | "settings"
  | "account"
  | "about"
  | "diagnostics";

/** 侧栏分组：研究工作区 / 管理。 */
export type NavigationGroup = "research" | "manage";
