import { memo } from "react";
import { Copy, GitBranch, Pencil } from "lucide-react";
import type { Message } from "../../bridge/desktop";
import CitationPanel from "./CitationPanel";
import ChatAgentTimeline from "./ChatAgentTimeline";
import ChatFollowUpForm from "./ChatFollowUpForm";
import ChatSuggestedQuestions, { deriveSuggestedQuestions } from "./ChatSuggestedQuestions";
import ChatTurnTimer from "./ChatTurnTimer";
import { evidenceToLiteratureResults } from "./chatEvidence";
import { useChatMessageActions } from "./chatMessageActions";
import { useMessageTimeline } from "./useMessageTimeline";
import { parseWebResponse, type WebPendingConfirmation } from "./web/webTypes";
import WebConfirmDialog from "./web/WebConfirmDialog";
import WebFeedback from "./web/WebFeedback";
import WebRecommendationCard from "./web/WebRecommendationCard";
import { Button } from "../../components/ui/button";

function isAssistant(message: Message) {
  return message.role === "agent" || message.role === "assistant";
}

function copyText(content: string) {
  void navigator.clipboard.writeText(content).catch(() => undefined);
}

function responseEvidence(message: Message) {
  const response = parseWebResponse(message);
  if (!response || response.evidence.length === 0) return null;
  try {
    const record = JSON.parse(message.response_json ?? "") as { evidence_pack_id?: unknown };
    return typeof record.evidence_pack_id === "string"
      ? { auditId: record.evidence_pack_id, evidence: response.evidence }
      : null;
  } catch {
    return null;
  }
}

function userContextChips(message: Message) {
  try {
    const record = JSON.parse(message.response_json ?? "") as {
      attachments?: Array<{ name?: string }>;
      knowledge_bases?: Array<{ name?: string }>;
      tags?: string[];
      images?: Array<{ name?: string }>;
    };
    const chips: string[] = [];
    record.attachments?.forEach((item) => { if (item.name) chips.push(item.name); });
    record.images?.forEach((item) => { if (item.name) chips.push(item.name); });
    record.knowledge_bases?.forEach((item) => { if (item.name) chips.push(`知识库 · ${item.name}`); });
    record.tags?.forEach((tag) => chips.push(tag));
    return chips;
  } catch {
    return [];
  }
}

const NativeChatMessage = memo(function NativeChatMessage({
  message,
  index,
  loading,
}: {
  message: Message;
  index: number;
  loading: boolean;
}) {
  const { onEditMessage, onFollowUp, onResolvePermission, onForkMessage, onFeedback } = useChatMessageActions();
  const response = parseWebResponse(message);
  const evidence = responseEvidence(message);
  const { steps } = useMessageTimeline({
    runId: response?.run_id ?? null,
    reasoning: response?.reasoning,
    reasoningMs: response?.reasoning_ms,
    toolCalls: response?.tool_calls,
  });

  if (!isAssistant(message)) {
    const chips = userContextChips(message);
    return (
      <div className="suna-chat-user-turn" data-agent-user-turn={index} data-message-id={message.id}>
        {chips.length > 0 ? (
          <div className="suna-chat-user-context" aria-label="本轮上下文">
            {chips.map((chip) => <span key={chip} className="suna-chat-user-context-chip">{chip}</span>)}
          </div>
        ) : null}
        <div className="suna-chat-user-bubble">{message.content}</div>
        <div className="suna-chat-message-actions">
          <button type="button" aria-label="复制消息" title="复制消息" onClick={() => void copyText(message.content)}>
            <Copy size={15} aria-hidden="true" />
          </button>
          <button type="button" aria-label="编辑消息" title="编辑消息" onClick={() => onEditMessage(message)}>
            <Pencil size={15} aria-hidden="true" />
          </button>
          {onForkMessage ? (
            <button type="button" aria-label="从此处分叉" title="从此处分叉" onClick={() => onForkMessage(message)}>
              <GitBranch size={15} aria-hidden="true" />
            </button>
          ) : null}
        </div>
      </div>
    );
  }

  const confirmations = response?.pending_confirmations ?? [];
  const suggested = response?.suggested_questions?.length
    ? response.suggested_questions
    : deriveSuggestedQuestions(message.content);

  const turnTimer = response?.started_at && response?.ended_at ? (
    <ChatTurnTimer
      active={false}
      startedAt={Date.parse(response.started_at) || null}
      endedAt={Date.parse(response.ended_at) || null}
    />
  ) : response?.duration_ms != null ? (
    <ChatTurnTimer
      active={false}
      startedAt={1}
      endedAt={1 + Math.max(0, response.duration_ms)}
    />
  ) : null;

  return (
    <article className="suna-chat-assistant-turn" aria-label="Suna" data-message-id={message.id}>
      {turnTimer}
      <ChatAgentTimeline
        steps={steps}
        finalAnswer={message.content}
        finalAnswerVisible
        streaming={false}
        literatureResults={evidenceToLiteratureResults(response?.evidence ?? [])}
        webSources={response?.web_sources ?? []}
        queryRewrite={response?.query_rewrite}
        research={response?.research}
        parentRunId={response?.run_id ?? null}
        parentConversationId={message.conversation_id}
        onResolvePermission={onResolvePermission}
      />
      {response && (response.context_status.memory_count > 0 || response.context_status.skill_count > 0) && (
        <div className="suna-chat-inline-status" aria-label="本轮上下文">
          <div className="suna-chat-inline-status-line">
            {response.context_status.memory_count > 0 && <span>{response.context_status.memory_count} 条记忆</span>}
            {response.context_status.skill_count > 0 && <span>{response.context_status.skill_count} 个技能</span>}
          </div>
        </div>
      )}
      {evidence && <CitationPanel auditId={evidence.auditId} evidence={evidence.evidence} />}
      {response?.follow_up_questions.length === 1 ? (
        <div className="suna-chat-inline-status" aria-label="需要补充的信息">
          <strong>需要补充的信息</strong>
          <div className="suna-chat-permission-actions">
            {response.follow_up_questions.map((question) => (
              <Button variant="actionSecondary" type="button" key={question} onClick={() => onFollowUp(question)}>{question}</Button>
            ))}
          </div>
        </div>
      ) : response?.follow_up_questions.length ? (
        <ChatFollowUpForm
          questions={response.follow_up_questions}
          disabled={loading}
          onSubmit={(answer) => onFollowUp(answer)}
        />
      ) : null}
      {!loading && suggested.length > 0 ? (
        <ChatSuggestedQuestions questions={suggested} onSelect={onFollowUp} />
      ) : null}
      {response?.recommendations.length ? (
        <div className="suna-chat-inline-status" aria-label="推荐方案">
          <strong>推荐方案</strong>
          <div className="grid gap-3 xl:grid-cols-2">
            {response.recommendations.map((item, itemIndex) => <WebRecommendationCard key={`${item.title}-${itemIndex}`} item={item} />)}
          </div>
        </div>
      ) : null}
      {confirmations.length > 0 && (
        <div className="suna-chat-inline-status">
          <WebConfirmDialog
            confirmations={confirmations}
            onConfirm={(item: WebPendingConfirmation, approved) => onResolvePermission(item.action_id, approved ? "allow_once" : "deny")}
          />
        </div>
      )}
      <div className="suna-chat-message-actions-row">
        {!loading && onFeedback ? (
          <WebFeedback
            messageId={message.id}
            conversationId={message.conversation_id}
            onFeedback={(rating, reason) => onFeedback(message, rating, reason)}
          />
        ) : null}
        <div className="suna-chat-message-actions">
          <button type="button" aria-label="复制回答" title="复制回答" onClick={() => void copyText(message.content)}>
            <Copy size={15} aria-hidden="true" />
          </button>
          {onForkMessage ? (
            <button type="button" aria-label="从此处分叉" title="从此处分叉" onClick={() => onForkMessage(message)}>
              <GitBranch size={15} aria-hidden="true" />
            </button>
          ) : null}
        </div>
      </div>
    </article>
  );
});

export default NativeChatMessage;
