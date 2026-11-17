import { describe, expect, it } from "vitest";
import { normalizeStreamingMarkdown } from "./streamingMarkdown";

describe("normalizeStreamingMarkdown", () => {
  it("closes unfinished bold markers while streaming", () => {
    expect(normalizeStreamingMarkdown("这是 **粗")).toContain("**");
  });

  it("strips a dangling list marker at the end", () => {
    expect(normalizeStreamingMarkdown("第一行\n-")).toBe("第一行\n");
  });
});
