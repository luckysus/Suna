import { BookOpen, Boxes, Cpu, FileText, Puzzle, Server } from "lucide-react";
import { desktop } from "../bridge/desktop";
import type { CommandMenuGroup, CommandMenuItem } from "../components/ui/command-menu";

/**
 * 第 72 章：全局搜索（Ctrl+K）的实体检索。
 *
 * 覆盖对话 / 知识库 / 文献 / Agent / Skill / MCP / 模型 / 设置：
 * - 知识库、Agent、Skill、MCP、模型属于「静态实体」，加载一次后本地过滤；
 * - 文献是后端检索，按查询实时拉取。
 */

export type GlobalEntityGroupId = "knowledge" | "agent" | "skill" | "mcp" | "model";

export type GlobalEntity = CommandMenuItem & {
  group: GlobalEntityGroupId;
  /** 参与匹配的额外文本（描述、模型 id 等）。 */
  keywords: string;
};

const GROUP_LABELS: Record<GlobalEntityGroupId, string> = {
  knowledge: "知识库",
  agent: "Agent",
  skill: "Skill",
  mcp: "MCP",
  model: "模型",
};

const GROUP_ORDER: GlobalEntityGroupId[] = ["knowledge", "agent", "skill", "mcp", "model"];

/** 加载可离线缓存的实体；单项失败不影响其余分组。 */
export async function loadGlobalEntities(): Promise<GlobalEntity[]> {
  const [bases, agents, skills, servers, models] = await Promise.all([
    desktop.listKnowledgeBases().catch(() => []),
    desktop.listAgentProfiles().catch(() => []),
    desktop.listSkills().catch(() => ({ skills: [], errors: [] })),
    desktop.listMcpServers().catch(() => []),
    desktop.listProviderProfiles().catch(() => []),
  ]);

  const entities: GlobalEntity[] = [];

  for (const base of bases) {
    entities.push({
      id: `knowledge:${base.id}`,
      title: base.name,
      subtitle: base.description || "知识库",
      section: "knowledge",
      icon: BookOpen,
      group: "knowledge",
      keywords: `${base.name} ${base.description ?? ""} ${(base.tags ?? []).join(" ")}`.toLowerCase(),
    });
  }
  for (const agent of agents) {
    entities.push({
      id: `agent:${agent.id}`,
      title: agent.name,
      subtitle: agent.description || (agent.preset ? "预置 Agent" : "自定义 Agent"),
      section: "agents",
      icon: Boxes,
      group: "agent",
      keywords: `${agent.name} ${agent.description} ${agent.id}`.toLowerCase(),
    });
  }
  for (const skill of skills.skills) {
    entities.push({
      id: `skill:${skill.name}`,
      title: skill.name,
      subtitle: skill.description || "Skill",
      section: "skills",
      icon: Puzzle,
      group: "skill",
      keywords: `${skill.name} ${skill.description} ${skill.tags.join(" ")}`.toLowerCase(),
    });
  }
  for (const server of servers) {
    entities.push({
      id: `mcp:${server.id}`,
      title: server.display_name,
      subtitle: `${server.transport} · ${server.enabled ? "已启用" : "已停用"}`,
      section: "mcp",
      icon: Server,
      group: "mcp",
      keywords: `${server.display_name} ${server.server_id} ${server.transport}`.toLowerCase(),
    });
  }
  for (const model of models) {
    entities.push({
      id: `model:${model.id}`,
      title: model.display_name,
      subtitle: model.model_id ? `${model.model_id} · ${model.kind}` : model.kind,
      section: "models",
      icon: Cpu,
      group: "model",
      keywords: `${model.display_name} ${model.model_id ?? ""} ${model.kind}`.toLowerCase(),
    });
  }

  return entities;
}

/** 按查询过滤实体，按分组输出（每组最多 limitPerGroup 条）。 */
export function filterGlobalEntities(
  entities: GlobalEntity[],
  query: string,
  limitPerGroup = 4,
): CommandMenuGroup[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [];
  const matched = entities.filter((entity) => entity.keywords.includes(needle));
  return GROUP_ORDER.map((group) => ({
    id: `entity-${group}`,
    label: GROUP_LABELS[group],
    items: matched.filter((entity) => entity.group === group).slice(0, limitPerGroup),
  })).filter((group) => group.items.length > 0);
}

/** 文献检索是后端调用，按查询实时拉取；失败时返回空。 */
export async function searchLiteratureItems(query: string, limit = 4): Promise<CommandMenuItem[]> {
  if (!query.trim()) return [];
  try {
    const response = await desktop.searchLiterature({ query: query.trim(), limit: 12 });
    const hits = response.results ?? response.literature_results ?? [];
    return hits.slice(0, limit).map((hit, index) => ({
      id: `literature:${hit.document_id ?? hit.source_name}:${index}`,
      title: hit.title_path || hit.source_name,
      subtitle: hit.snippet || "文献证据",
      section: "literature" as const,
      icon: FileText,
      payload: hit,
    }));
  } catch {
    return [];
  }
}
