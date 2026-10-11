import { describe, expect, it } from "vitest";
import { filterGlobalEntities, type GlobalEntity } from "./globalSearch";

function entity(overrides: Partial<GlobalEntity> & { id: string; title: string; group: GlobalEntity["group"] }): GlobalEntity {
  return {
    subtitle: "",
    keywords: `${overrides.title}`.toLowerCase(),
    ...overrides,
  };
}

const entities: GlobalEntity[] = [
  entity({ id: "k1", title: "耐磨钢知识库", group: "knowledge" }),
  entity({ id: "k2", title: "IF 钢知识库", group: "knowledge" }),
  entity({ id: "a1", title: "Knowledge Agent", group: "agent" }),
  entity({ id: "s1", title: "paper-summary", group: "skill" }),
  entity({ id: "m1", title: "filesystem", group: "mcp" }),
  entity({ id: "p1", title: "DeepSeek V4", group: "model" }),
];

describe("filterGlobalEntities", () => {
  it("returns nothing for an empty query", () => {
    expect(filterGlobalEntities(entities, "   ")).toEqual([]);
  });

  it("groups matches by entity type with labels", () => {
    const groups = filterGlobalEntities(entities, "钢");
    expect(groups).toHaveLength(1);
    expect(groups[0].label).toBe("知识库");
    expect(groups[0].items.map((item) => item.title)).toEqual(["耐磨钢知识库", "IF 钢知识库"]);
  });

  it("matches case-insensitively across groups", () => {
    const groups = filterGlobalEntities(entities, "agent");
    expect(groups.map((group) => group.label)).toEqual(["Agent"]);
    expect(groups[0].items[0].title).toBe("Knowledge Agent");
  });

  it("caps the number of items per group", () => {
    const groups = filterGlobalEntities(entities, "知识库", 1);
    expect(groups[0].items).toHaveLength(1);
  });

  it("keeps the documented group order", () => {
    const groups = filterGlobalEntities(entities, "e");
    const labels = groups.map((group) => group.label);
    const expectedOrder = ["知识库", "Agent", "Skill", "MCP", "模型"];
    expect(labels).toEqual(expectedOrder.filter((label) => labels.includes(label)));
  });
});
