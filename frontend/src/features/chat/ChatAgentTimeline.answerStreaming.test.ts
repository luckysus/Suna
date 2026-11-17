import { describe, expect, it } from "vitest";
import { isAnswerStillStreaming } from "./ChatAgentTimeline";
import type { ChatTimelineStep } from "./chatTimeline";

const completedFinal: ChatTimelineStep = {
  id: "c1",
  kind: "content",
  detail: "最终答案",
  status: "completed",
  isFinal: true,
};

describe("isAnswerStillStreaming", () => {
  it("is false once the model final already completed in the timeline", () => {
    expect(isAnswerStillStreaming([completedFinal], true, "最终答案")).toBe(false);
  });

  it("is false for settled history turns", () => {
    expect(isAnswerStillStreaming([completedFinal], false, "最终答案")).toBe(false);
  });

  it("is true only while streaming an unfinished bottom answer", () => {
    const running: ChatTimelineStep = {
      id: "c2",
      kind: "content",
      detail: "半",
      status: "running",
    };
    expect(isAnswerStillStreaming([running], true, "半")).toBe(true);
  });
});
