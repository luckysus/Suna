import {
  BookOpen,
  BrainCircuit,
  ChartNoAxesCombined,
  CircleHelp,
  Cpu,
  FileChartColumn,
  FlaskConical,
  FolderKanban,
  Gauge,
  Library,
  MessageSquareText,
  Network,
  Settings2,
  Sparkles,
  Table2,
  UserRound,
  Wrench,
} from "lucide-react";
import type { LucideIcon } from "lucide-react";
import type { MessageKey } from "../i18n/locale";
import type { NavigationGroup, SectionId } from "../types/navigation";

export type { NavigationGroup, SectionId } from "../types/navigation";
export interface NavigationSection {
  id: SectionId;
  label: string;
  description: string;
  icon: LucideIcon;
  group: NavigationGroup;
  labelKey: MessageKey;
  descriptionKey: MessageKey;
}

/** 核心研究工作区（基于开发文档与第四/五部分扩展） */
export const primaryNavigationSections: readonly NavigationSection[] = [
  { id: "chat", label: "对话中心", description: "与 Suna 协作完成材料研发任务", icon: MessageSquareText, group: "research", labelKey: "navChat", descriptionKey: "navChatDesc" },
  { id: "projects", label: "科研项目", description: "组织课题、任务与成果来源", icon: FolderKanban, group: "research", labelKey: "navProjects", descriptionKey: "navProjectsDesc" },
  { id: "knowledge", label: "知识中心", description: "管理知识库、文档和 Wiki", icon: Library, group: "research", labelKey: "navKnowledge", descriptionKey: "navKnowledgeDesc" },
  { id: "literature", label: "文献研究", description: "检索、阅读和总结科研文献", icon: BookOpen, group: "research", labelKey: "navLiterature", descriptionKey: "navLiteratureDesc" },
  { id: "data", label: "数据实验室", description: "处理数据并生成科研图表", icon: Table2, group: "research", labelKey: "navData", descriptionKey: "navDataDesc" },
  { id: "prediction", label: "性能预测", description: "预测材料性能并解释模型", icon: ChartNoAxesCombined, group: "research", labelKey: "navPrediction", descriptionKey: "navPredictionDesc" },
  { id: "optimization", label: "工艺优化", description: "探索多目标工艺方案", icon: Gauge, group: "research", labelKey: "navOptimization", descriptionKey: "navOptimizationDesc" },
  { id: "experiment", label: "实验助手", description: "设计下一组高价值实验", icon: FlaskConical, group: "research", labelKey: "navExperiment", descriptionKey: "navExperimentDesc" },
];

/** 侧边栏实际展示的真实能力与管理模块（剔除设置借壳项与重复项） */
export const sidebarManageSections: readonly NavigationSection[] = [
  { id: "agents", label: "Agent 管理", description: "配置主 Agent 和专业 Agent", icon: BrainCircuit, group: "manage", labelKey: "navAgents", descriptionKey: "navAgentsDesc" },
  { id: "tools", label: "工具中心", description: "管理文件、计算和分析工具", icon: Wrench, group: "manage", labelKey: "navTools", descriptionKey: "navToolsDesc" },
  { id: "mcp", label: "MCP 管理", description: "连接外部工具和数据服务", icon: Network, group: "manage", labelKey: "navMcp", descriptionKey: "navMcpDesc" },
  { id: "skills", label: "Skill 管理", description: "管理可复用的专业能力", icon: Sparkles, group: "manage", labelKey: "navSkills", descriptionKey: "navSkillsDesc" },
  { id: "reports", label: "科研报告", description: "生成报告、综述和实验记录", icon: FileChartColumn, group: "manage", labelKey: "navReports", descriptionKey: "navReportsDesc" },
];

/** 全量辅助功能定义（供路由与 Ctrl+K 全局搜索快速跳转，保留设置相关条目） */
export const utilityNavigationSections: readonly NavigationSection[] = [
  ...sidebarManageSections,
  { id: "settings", label: "设置中心", description: "配置模型、外观和本地数据", icon: Settings2, group: "manage", labelKey: "navSettings", descriptionKey: "navSettingsDesc" },
  { id: "models", label: "模型服务", description: "配置对话、Embedding 和重排模型", icon: Cpu, group: "manage", labelKey: "navModels", descriptionKey: "navModelsDesc" },
  { id: "account", label: "个人账户", description: "管理本地账户和个人偏好", icon: UserRound, group: "manage", labelKey: "navAccount", descriptionKey: "navAccountDesc" },
  { id: "about", label: "关于 Suna", description: "Suna 版本和项目信息", icon: CircleHelp, group: "manage", labelKey: "navAbout", descriptionKey: "navAboutDesc" },
  { id: "diagnostics", label: "运行记录", description: "查看 Agent 任务和系统状态", icon: Settings2, group: "manage", labelKey: "navDiagnostics", descriptionKey: "navDiagnosticsDesc" },
];

export const navigationSections = [...primaryNavigationSections, ...utilityNavigationSections] as const;
export function getNavigationSection(id: SectionId) {
  return navigationSections.find((section) => section.id === id) ?? navigationSections[0];
}

export function getLocalizedSection(section: NavigationSection, t: (key: MessageKey) => string): NavigationSection {
  return {
    ...section,
    label: t(section.labelKey),
    description: t(section.descriptionKey),
  };
}
