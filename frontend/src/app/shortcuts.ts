/**
 * 第 73 章：全局快捷键定义。
 *
 * `SunaApp`（按键处理）与 `SettingsShortcutsPanel`（设置面板）共用这份定义，
 * 避免两边各写一份导致漂移。
 */

export type ShortcutKey =
  | "newConversation"
  | "openCommand"
  | "openKnowledge"
  | "focusInput"
  | "send"
  | "stopAgent"
  | "openSettings"
  | "searchHistory"
  | "toggleSidebar"
  | "toggleInspector"
  | "recoverTask";

export type ShortcutSettings = Record<ShortcutKey, string>;

/** 文档第 73 章要求：Ctrl+N / Ctrl+K / Ctrl+Shift+K / Ctrl+Enter / Esc，其余为产品增强项。 */
export const defaultShortcuts: ShortcutSettings = {
  newConversation: "Ctrl+N",
  openCommand: "Ctrl+K",
  openKnowledge: "Ctrl+Shift+K",
  focusInput: "Ctrl+L",
  send: "Ctrl+Enter",
  stopAgent: "Esc",
  openSettings: "Ctrl+,",
  searchHistory: "Ctrl+Shift+F",
  toggleSidebar: "Ctrl+\\",
  toggleInspector: "Ctrl+J",
  recoverTask: "Ctrl+Shift+R",
};

export const shortcutRows: Array<{ key: ShortcutKey; label: string; copy: string }> = [
  { key: "newConversation", label: "新建对话", copy: "创建一个新的 Agent 会话" },
  { key: "openCommand", label: "全局搜索", copy: "搜索对话、知识库、文献、Agent、Skill、MCP、模型与设置" },
  { key: "openKnowledge", label: "知识库", copy: "直接进入知识中心" },
  { key: "focusInput", label: "聚焦输入框", copy: "快速回到消息编辑区" },
  { key: "send", label: "发送消息", copy: "提交当前消息或草稿" },
  { key: "stopAgent", label: "停止 Agent", copy: "中断正在运行的任务" },
  { key: "openSettings", label: "打开设置", copy: "直接进入设置中心" },
  { key: "searchHistory", label: "搜索对话", copy: "在历史会话中查找内容" },
  { key: "toggleSidebar", label: "切换侧栏", copy: "显示或隐藏主导航" },
  { key: "toggleInspector", label: "切换运行检查器", copy: "显示或隐藏 Agent 运行面板" },
  { key: "recoverTask", label: "恢复最近任务", copy: "继续上次中断的 Agent 运行" },
];

export const shortcutChoices = [
  "Ctrl+N",
  "Ctrl+K",
  "Ctrl+Shift+K",
  "Ctrl+L",
  "Ctrl+Enter",
  "Enter",
  "Esc",
  "Ctrl+,",
  "Ctrl+Shift+F",
  "Ctrl+\\",
  "Ctrl+J",
  "Ctrl+Shift+R",
  "未设置",
];

/** 把持久化值规整为完整的快捷键表；缺失项回落到默认值。 */
export function normalizeShortcuts(raw: string | null, fallback: ShortcutSettings = defaultShortcuts): ShortcutSettings {
  if (!raw) return fallback;
  try {
    const value = JSON.parse(raw) as Record<string, unknown>;
    return Object.fromEntries(
      shortcutRows.map(({ key }) => [key, typeof value[key] === "string" ? (value[key] as string) : fallback[key]]),
    ) as ShortcutSettings;
  } catch {
    return fallback;
  }
}

/** 把 "Ctrl+Shift+K" 解析成可比较的按键描述。 */
export function parseShortcut(shortcut: string): { key: string; ctrl: boolean; shift: boolean; alt: boolean; meta: boolean } {
  const parts = shortcut.split("+").map((part) => part.trim()).filter(Boolean);
  const key = parts.length > 0 ? parts[parts.length - 1] : "";
  const modifiers = parts.slice(0, -1).map((part) => part.toLowerCase());
  return {
    key,
    ctrl: modifiers.includes("ctrl"),
    shift: modifiers.includes("shift"),
    alt: modifiers.includes("alt"),
    meta: modifiers.includes("meta") || modifiers.includes("cmd"),
  };
}
