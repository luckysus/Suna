import { describe, expect, it } from "vitest";
import type { AgentEventData, AgentEventEnvelope } from "../../bridge/generated/protocol";
import {
  applyTimelineEvent,
  buildTimelineFromEvents,
  finalAnswerFromTimeline,
  visibleTimelineSteps,
} from "./chatTimeline";

const runId = "run-1";
const conversationId = "conversation-1";

function event(sequence: number, data: AgentEventData): AgentEventEnvelope {
  return {
    protocol_version: 1,
    event_id: `event-${sequence}`,
    run_id: runId,
    conversation_id: conversationId,
    sequence,
    timestamp: "2026-08-07T10:00:00Z",
    ...data,
  };
}

describe("chatTimeline", () => {
  it("interleaves Thought and interim content across tool rounds", () => {
    const steps = buildTimelineFromEvents([
      event(1, { type: "reasoning_delta", data: { message_id: "m1", delta: "先查工具。" } }),
      event(2, { type: "reasoning_completed", data: { message_id: "m1", duration_ms: 1200 } }),
      event(3, { type: "message_delta", data: { message_id: "m1", role: "assistant", delta: "中间说明。" } }),
      event(4, { type: "message_completed", data: { message_id: "m1", role: "assistant", content: "中间说明。", partial: true } }),
      event(5, { type: "tool_requested", data: { tool_call_id: "t1", tool_id: "steel.lookup", tool_name: "Lookup", arguments: {} } }),
      event(6, { type: "reasoning_delta", data: { message_id: "m1", delta: "整理结论。" } }),
      event(7, { type: "reasoning_completed", data: { message_id: "m1", duration_ms: 800 } }),
      event(8, { type: "message_delta", data: { message_id: "m1", role: "assistant", delta: "最终答案。" } }),
      event(9, { type: "message_completed", data: { message_id: "m1", role: "assistant", content: "最终答案。", partial: false } }),
      event(10, { type: "run_completed", data: { outcome: "completed", assistant_message_id: "m1" } }),
    ]);

    expect(steps.map((step) => ({
      kind: step.kind,
      detail: step.kind === "tool" ? step.name : step.detail,
      isFinal: step.kind === "content" ? step.isFinal : undefined,
    }))).toEqual([
      { kind: "thought", detail: "先查工具。", isFinal: undefined },
      { kind: "content", detail: "中间说明。", isFinal: false },
      { kind: "tool", detail: "Lookup", isFinal: undefined },
      { kind: "thought", detail: "整理结论。", isFinal: undefined },
      { kind: "content", detail: "最终答案。", isFinal: true },
    ]);
    expect(steps[0]).toMatchObject({ status: "completed", durationMs: 1200 });
    expect(finalAnswerFromTimeline(steps, "", true)).toBe("最终答案。");
    expect(visibleTimelineSteps(steps, true).map((step) => step.kind)).toEqual(["thought", "content", "tool", "thought"]);
  });

  it("keeps streaming content in the timeline until the run settles", () => {
    let steps = [] as ReturnType<typeof applyTimelineEvent>;
    steps = applyTimelineEvent(steps, event(1, { type: "reasoning_delta", data: { message_id: "m1", delta: "思考" } }));
    steps = applyTimelineEvent(steps, event(2, { type: "reasoning_completed", data: { message_id: "m1", duration_ms: 500 } }));
    steps = applyTimelineEvent(steps, event(3, { type: "message_delta", data: { message_id: "m1", role: "assistant", delta: "答" } }));
    steps = applyTimelineEvent(steps, event(4, { type: "message_delta", data: { message_id: "m1", role: "assistant", delta: "案" } }));

    expect(steps).toHaveLength(2);
    expect(steps[1]).toMatchObject({ kind: "content", detail: "答案", status: "running" });
    expect(visibleTimelineSteps(steps, false)).toHaveLength(2);
    expect(finalAnswerFromTimeline(steps, "答案", false)).toBe("");
  });

  it("coalesces consecutive reasoning bursts into one Thought panel", () => {
    let steps = [] as ReturnType<typeof applyTimelineEvent>;
    steps = applyTimelineEvent(steps, event(1, { type: "reasoning_delta", data: { message_id: "m1", delta: "先想" } }));
    steps = applyTimelineEvent(steps, event(2, { type: "reasoning_completed", data: { message_id: "m1", duration_ms: 200 } }));
    steps = applyTimelineEvent(steps, event(3, { type: "reasoning_delta", data: { message_id: "m1", delta: "再想" } }));
    steps = applyTimelineEvent(steps, event(4, { type: "reasoning_completed", data: { message_id: "m1", duration_ms: 300 } }));
    steps = applyTimelineEvent(steps, event(5, { type: "message_delta", data: { message_id: "m1", role: "assistant", delta: "" } }));
    steps = applyTimelineEvent(steps, event(6, { type: "reasoning_delta", data: { message_id: "m1", delta: "继续" } }));

    expect(steps).toHaveLength(1);
    expect(steps[0]).toMatchObject({ kind: "thought", detail: "先想再想继续", status: "running", durationMs: 500 });
  });

  it("only collapses a thought when content or tools interrupt it", () => {
    let steps = [] as ReturnType<typeof applyTimelineEvent>;
    steps = applyTimelineEvent(steps, event(1, { type: "reasoning_delta", data: { message_id: "m1", delta: "思考" } }));
    steps = applyTimelineEvent(steps, event(2, { type: "reasoning_completed", data: { message_id: "m1", duration_ms: 900 } }));
    expect(steps[0]).toMatchObject({ status: "running", durationMs: 900 });

    steps = applyTimelineEvent(steps, event(3, { type: "message_delta", data: { message_id: "m1", role: "assistant", delta: "答案" } }));
    expect(steps[0]).toMatchObject({ kind: "thought", status: "completed", durationMs: 900 });
    expect(steps[1]).toMatchObject({ kind: "content", detail: "答案", status: "running" });
  });
});
