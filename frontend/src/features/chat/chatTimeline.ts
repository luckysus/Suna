import type { AgentError, AgentEventEnvelope, ToolOutcome } from "../../bridge/generated/protocol";

export type ChatTimelineStep =
  | {
      id: string;
      kind: "thought";
      detail: string;
      status: "running" | "completed";
      durationMs?: number | null;
    }
  | {
      id: string;
      kind: "content";
      detail: string;
      status: "running" | "completed";
      isFinal?: boolean;
    }
  | {
      id: string;
      kind: "tool";
      toolCallId: string;
      toolId: string;
      name: string;
      status: "requested" | "running" | ToolOutcome;
      progress: number;
      message: string | null;
      error: AgentError | null;
    };

function sealRunning(
  steps: ChatTimelineStep[],
  kinds: Array<"thought" | "content"> = ["thought", "content"],
): ChatTimelineStep[] {
  if (steps.length === 0) return steps;
  const last = steps[steps.length - 1];
  if (last.status !== "running" || (last.kind !== "thought" && last.kind !== "content") || !kinds.includes(last.kind)) {
    return steps;
  }
  return [...steps.slice(0, -1), { ...last, status: "completed" }];
}

function updateToolStep(
  steps: ChatTimelineStep[],
  toolCallId: string,
  patch: Partial<Extract<ChatTimelineStep, { kind: "tool" }>>,
): ChatTimelineStep[] {
  return steps.map((step) =>
    step.kind === "tool" && step.toolCallId === toolCallId ? { ...step, ...patch } : step,
  );
}

/** Build interleaved Thought / content / tool steps from Suna agent protocol events. */
export function applyTimelineEvent(
  steps: ChatTimelineStep[],
  event: AgentEventEnvelope,
): ChatTimelineStep[] {
  switch (event.type) {
    case "reasoning_delta": {
      const delta = event.data.delta;
      if (!delta) return steps;
      const next = sealRunning(steps, ["content"]);
      const last = next[next.length - 1];
      // Keep one Thought panel per reasoning span. Only start a new one after
      // content / tool / status has interrupted the previous thought.
      if (last?.kind === "thought") {
        return [
          ...next.slice(0, -1),
          {
            ...last,
            detail: `${last.detail}${delta}`,
            status: "running",
          },
        ];
      }
      return [
        ...next,
        {
          id: `thought-${event.event_id}`,
          kind: "thought",
          detail: delta,
          status: "running",
        },
      ];
    }
    case "reasoning_completed": {
      const last = steps[steps.length - 1];
      if (last?.kind !== "thought") return steps;
      if (!last.detail.trim()) {
        return steps.slice(0, -1);
      }
      // Keep status=running until content/tool/run seals the thought.
      // Completing here caused expand/collapse flicker when more reasoning followed.
      return [
        ...steps.slice(0, -1),
        {
          ...last,
          durationMs: (last.durationMs ?? 0) + Math.max(1, event.data.duration_ms),
        },
      ];
    }
    case "message_delta": {
      if (event.data.role !== "assistant") return steps;
      const delta = event.data.delta;
      if (!delta) return steps;
      const next = sealRunning(steps, ["thought"]);
      const last = next[next.length - 1];
      if (last?.kind === "content" && last.status === "running") {
        return [...next.slice(0, -1), { ...last, detail: `${last.detail}${delta}` }];
      }
      return [
        ...next,
        {
          id: `content-${event.event_id}`,
          kind: "content",
          detail: delta,
          status: "running",
        },
      ];
    }
    case "message_completed": {
      if (event.data.role !== "assistant") return steps;
      const sealed = sealRunning(steps, ["thought"]);
      const last = sealed[sealed.length - 1];
      const content = event.data.content?.trim() ? event.data.content : last?.kind === "content" ? last.detail : "";
      if (last?.kind === "content") {
        return [
          ...sealed.slice(0, -1),
          {
            ...last,
            detail: content,
            status: "completed",
            isFinal: !event.data.partial,
          },
        ];
      }
      if (!content) return sealed;
      return [
        ...sealed,
        {
          id: `content-${event.event_id}`,
          kind: "content",
          detail: content,
          status: "completed",
          isFinal: !event.data.partial,
        },
      ];
    }
    case "tool_requested": {
      const next = sealRunning(steps);
      return [
        ...next,
        {
          id: `tool-${event.data.tool_call_id}`,
          kind: "tool",
          toolCallId: event.data.tool_call_id,
          toolId: event.data.tool_id,
          name: event.data.tool_name,
          status: "requested",
          progress: 0,
          message: null,
          error: null,
        },
      ];
    }
    case "tool_started":
      return updateToolStep(steps, event.data.tool_call_id, { status: "running" });
    case "tool_progress":
      return updateToolStep(steps, event.data.tool_call_id, {
        progress: event.data.progress,
        message: event.data.message,
      });
    case "tool_completed":
      return updateToolStep(steps, event.data.tool_call_id, {
        status: event.data.outcome,
        progress: event.data.outcome === "succeeded" ? 100 : undefined,
        error: event.data.error,
      });
    case "run_completed": {
      const sealed = sealRunning(steps);
      const lastContent = [...sealed].reverse().find((step) => step.kind === "content");
      if (!lastContent || lastContent.isFinal) return sealed;
      return sealed.map((step) =>
        step.id === lastContent.id ? { ...step, isFinal: true, status: "completed" } : step,
      );
    }
    default:
      return steps;
  }
}

export function buildTimelineFromEvents(events: AgentEventEnvelope[]): ChatTimelineStep[] {
  return [...events]
    .sort((left, right) => left.sequence - right.sequence)
    .reduce(applyTimelineEvent, [] as ChatTimelineStep[]);
}

/** Hide the final answer segment from the timeline once it is shown below. */
export function visibleTimelineSteps(
  steps: ChatTimelineStep[],
  finalAnswerVisible: boolean,
): ChatTimelineStep[] {
  if (!finalAnswerVisible) return steps;
  let latestFinal = -1;
  steps.forEach((step, index) => {
    if (step.kind === "content" && step.isFinal) latestFinal = index;
  });
  if (latestFinal < 0) {
    const lastContent = [...steps].map((step, index) => ({ step, index })).reverse()
      .find((entry) => entry.step.kind === "content");
    if (!lastContent) return steps;
    return steps.filter((_, index) => index !== lastContent.index);
  }
  return steps.filter((_, index) => index !== latestFinal);
}

export function finalAnswerFromTimeline(
  steps: ChatTimelineStep[],
  assistantText: string,
  settled: boolean,
): string {
  if (!settled) return "";
  const finals = steps.filter((step) => step.kind === "content" && step.isFinal);
  if (finals.length > 0) return finals[finals.length - 1].detail.trim();
  const contents = steps.filter((step) => step.kind === "content");
  if (contents.length > 0) return contents[contents.length - 1].detail.trim();
  return assistantText.trim();
}

export function timelineFromPersistedReasoning(
  reasoning: string | undefined,
  reasoningMs?: number | null,
): ChatTimelineStep[] {
  const detail = reasoning?.trim() ?? "";
  if (!detail) return [];
  return [
    {
      id: "persisted-thought",
      kind: "thought",
      detail,
      status: "completed",
      durationMs: reasoningMs ?? null,
    },
  ];
}

export function timelineFromPersistedToolCalls(toolCalls: unknown): ChatTimelineStep[] {
  if (!Array.isArray(toolCalls)) return [];
  return toolCalls.flatMap((item, index) => {
    if (!item || typeof item !== "object") return [];
    const record = item as Record<string, unknown>;
    const name = typeof record.tool_name === "string"
      ? record.tool_name
      : typeof record.name === "string"
        ? record.name
        : typeof record.tool_id === "string"
          ? record.tool_id
          : `工具 ${index + 1}`;
    const toolCallId = typeof record.tool_call_id === "string"
      ? record.tool_call_id
      : typeof record.id === "string"
        ? record.id
        : `persisted-tool-${index}`;
    const statusRaw = typeof record.status === "string" ? record.status : typeof record.outcome === "string" ? record.outcome : "succeeded";
    const status = (["requested", "running", "succeeded", "failed", "cancelled"].includes(statusRaw)
      ? statusRaw
      : "succeeded") as Extract<ChatTimelineStep, { kind: "tool" }>["status"];
    return [{
      id: `persisted-tool-${toolCallId}`,
      kind: "tool" as const,
      toolCallId,
      toolId: typeof record.tool_id === "string" ? record.tool_id : toolCallId,
      name,
      status,
      progress: typeof record.progress === "number" ? record.progress : status === "succeeded" ? 100 : 0,
      message: typeof record.message === "string" ? record.message : null,
      error: null,
    }];
  });
}
