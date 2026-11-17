import { memo, useMemo } from "react";
import { LoaderCircle, Wrench } from "lucide-react";
import LazyAIAnswerRenderer from "../../components/answer/LazyAIAnswerRenderer";
import { useTypewriter } from "../../hooks/useTypewriter";
import type { PermissionDecision } from "../../bridge/generated/protocol";
import type { AnswerReferenceResult } from "../../components/answer/AnswerRenderer";
import { useAppearanceSettings } from "../../settings/appearance";
import ChatThoughtPanel from "./ChatThoughtPanel";
import ChildAgentPanel from "./ChildAgentPanel";
import { QueryRewriteBar, ResearchTracePanel, WebSourcesBar } from "./ChatWebExtras";
import { normalizeStreamingMarkdown } from "./streamingMarkdown";
import {
  finalAnswerFromTimeline,
  visibleTimelineSteps,
  type ChatTimelineStep,
} from "./chatTimeline";
import type { WebQueryRewrite, WebResearchSnapshot, WebSource } from "./web/webTypes";

function toolStatusLabel(step: Extract<ChatTimelineStep, { kind: "tool" }>) {
  if (step.status === "succeeded") return "已完成";
  if (step.status === "failed") return "失败";
  if (step.status === "cancelled") return "已取消";
  if (step.status === "running") return step.progress > 0 ? `${step.progress}%` : "执行中";
  return "等待执行";
}

const TimelineToolStep = memo(function TimelineToolStep({
  step,
}: {
  step: Extract<ChatTimelineStep, { kind: "tool" }>;
}) {
  const running = step.status === "running" || step.status === "requested";
  return (
    <div className="suna-chat-timeline-tool" data-tool-status={step.status}>
      <span className={`suna-chat-timeline-tool-icon ${running ? "is-pending" : step.status === "succeeded" ? "is-good" : "is-danger"}`}>
        {running ? <LoaderCircle size={14} className="suna-spin" aria-hidden="true" /> : <Wrench size={14} aria-hidden="true" />}
      </span>
      <div className="suna-chat-timeline-tool-body">
        <strong className={running ? "is-active" : undefined}>工具 · {step.name}</strong>
        <span>{toolStatusLabel(step)}</span>
        {step.message ? <small>{step.message}</small> : null}
        {step.error ? <small>{step.error.code}: {step.error.message}</small> : null}
      </div>
    </div>
  );
});

const TimelineContentStep = memo(function TimelineContentStep({
  step,
  literatureResults = [],
}: {
  step: Extract<ChatTimelineStep, { kind: "content" }>;
  literatureResults?: AnswerReferenceResult[];
}) {
  const text = step.detail.trim();
  if (!text) return null;
  return (
    <div className="suna-chat-timeline-content ai-markdown-body" data-timeline-content={step.status}>
      <LazyAIAnswerRenderer answer={text} literatureResults={literatureResults} />
      {step.status === "running" ? <span className="ai-typing-cursor" aria-hidden="true" /> : null}
    </div>
  );
});

/** Interleaved Thought ↔ tool ↔ interim content — mirrors steel-agent AgentTimeline. */
export default function ChatAgentTimeline({
  steps,
  finalAnswer = "",
  finalAnswerVisible = Boolean(finalAnswer.trim()),
  streaming = false,
  emptyFallback = "",
  literatureResults = [],
  webSources = [],
  queryRewrite = null,
  research = null,
  parentRunId = null,
  parentConversationId = null,
  onResolvePermission,
}: {
  steps: ChatTimelineStep[];
  finalAnswer?: string;
  finalAnswerVisible?: boolean;
  streaming?: boolean;
  emptyFallback?: string;
  literatureResults?: AnswerReferenceResult[];
  webSources?: WebSource[];
  queryRewrite?: WebQueryRewrite | null;
  research?: WebResearchSnapshot | null;
  parentRunId?: string | null;
  parentConversationId?: string | null;
  onResolvePermission?: (permissionId: string, decision: PermissionDecision) => void;
}) {
  const visible = visibleTimelineSteps(steps, finalAnswerVisible).filter((step) =>
    step.kind === "thought" || step.kind === "content"
      ? step.detail.trim().length > 0
      : true,
  );
  const rawBottom = finalAnswerVisible
    ? finalAnswer.trim() || finalAnswerFromTimeline(steps, "", true)
    : "";
  // Text already finished in the timeline must not start a second typewriter
  // when it relocates to the answer container (steel-agent parity).
  const answerStreaming = isAnswerStillStreaming(steps, streaming, rawBottom);
  const normalizedBottom = answerStreaming && rawBottom
    ? normalizeStreamingMarkdown(rawBottom)
    : rawBottom;
  const bottom = useTypewriter(normalizedBottom, !answerStreaming);

  const showPreparing = streaming
    && !bottom.trim()
    && !visible.some((step) => (step.kind === "content" || step.kind === "tool") && (step.kind === "tool" || step.detail.trim()));

  const { preferences } = useAppearanceSettings();
  const childPanel = useMemo(() => {
    if (!parentRunId || !parentConversationId || !onResolvePermission) return null;
    return (
      <ChildAgentPanel
        parentRunId={parentRunId}
        parentConversationId={parentConversationId}
        showToolDetails={preferences.showToolDetails}
        onResolvePermission={onResolvePermission}
      />
    );
  }, [parentRunId, parentConversationId, onResolvePermission, preferences.showToolDetails]);

  return (
    <div className="suna-chat-timeline" data-agent-timeline>
      <QueryRewriteBar rewrite={queryRewrite} />
      <WebSourcesBar sources={webSources} />
      {visible.map((step) =>
        step.kind === "thought" ? (
          <ChatThoughtPanel
            key={step.id}
            reasoning={step.detail}
            reasoningMs={step.durationMs}
            streaming={step.status === "running"}
          />
        ) : step.kind === "tool" ? (
          <TimelineToolStep key={step.id} step={step} />
        ) : (
          <TimelineContentStep key={step.id} step={step} literatureResults={literatureResults} />
        ),
      )}
      {childPanel}
      <ResearchTracePanel snapshot={research} />
      {bottom || showPreparing ? (
        <div className="suna-chat-answer ai-markdown-body" data-agent-answer>
          <LazyAIAnswerRenderer
            answer={bottom || emptyFallback}
            literatureResults={literatureResults}
          />
          {answerStreaming && bottom ? <span className="ai-typing-cursor" aria-hidden="true" /> : null}
        </div>
      ) : null}
    </div>
  );
}

/** True while the bottom answer is still growing and has not finished once in the timeline. */
export function isAnswerStillStreaming(
  steps: ChatTimelineStep[],
  streaming: boolean,
  bottomAnswer: string,
): boolean {
  if (!streaming) return false;
  if (!bottomAnswer.trim()) return false;
  return !steps.some((step) => step.kind === "content" && step.isFinal && step.status === "completed");
}

