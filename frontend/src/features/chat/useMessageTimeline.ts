import { useEffect, useState } from "react";
import { desktop } from "../../bridge/desktop";
import {
  buildTimelineFromEvents,
  timelineFromPersistedReasoning,
  timelineFromPersistedToolCalls,
  type ChatTimelineStep,
} from "./chatTimeline";

const cache = new Map<string, ChatTimelineStep[]>();

export function useMessageTimeline(options: {
  runId: string | null;
  reasoning?: string;
  reasoningMs?: number | null;
  toolCalls?: unknown;
}): { steps: ChatTimelineStep[]; loading: boolean } {
  const { runId, reasoning, reasoningMs, toolCalls } = options;
  const fallback = [
    ...timelineFromPersistedReasoning(reasoning, reasoningMs),
    ...timelineFromPersistedToolCalls(toolCalls),
  ];
  const [steps, setSteps] = useState<ChatTimelineStep[]>(() =>
    runId && cache.has(runId) ? cache.get(runId)! : fallback,
  );
  const [loading, setLoading] = useState(Boolean(runId && !cache.has(runId)));

  useEffect(() => {
    if (!runId) {
      setSteps(fallback);
      setLoading(false);
      return;
    }
    if (cache.has(runId)) {
      setSteps(cache.get(runId)!);
      setLoading(false);
      return;
    }
    let active = true;
    setLoading(true);
    void desktop.replayAgentRun(runId, 0).then((events) => {
      if (!active) return;
      const next = buildTimelineFromEvents(events);
      const resolved = next.length > 0 ? next : fallback;
      cache.set(runId, resolved);
      setSteps(resolved);
      setLoading(false);
    }).catch(() => {
      if (!active) return;
      setSteps(fallback);
      setLoading(false);
    });
    return () => { active = false; };
  }, [runId, reasoning, reasoningMs, toolCalls]);

  return { steps, loading };
}
