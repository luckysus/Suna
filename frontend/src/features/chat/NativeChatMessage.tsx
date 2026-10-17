import { memo } from "react";
import { useChatMessageActions } from "./chatMessageActions";
import { Copy, Pencil } from "lucide-react";
import type { Message } from "../../bridge/desktop";
import LazyAIAnswerRenderer from "../../components/answer/LazyAIAnswerRenderer";
import CitationPanel from "./CitationPanel";
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

const NativeChatMessage = memo(function NativeChatMessage({
  message,
  index,
  loading,
}: {
  message: Message;
  index: number;
  loading: boolean;
}) {
  const { onEditMessage, onFollowUp, onResolvePermission } = useChatMessageActions();
  const response = parseWebResponse(message);
  const evidence = responseEvidence(message);

  if (!isAssistant(message)) {
    return (
      <div className="suna-chat-user-turn" data-agent-user-turn={index} data-message-id={message.id}>
        <div className="suna-chat-user-bubble">{message.content}</div>
        <div className="suna-chat-message-actions">
          <button type="button" aria-label="复制消息" title="复制消息" onClick={() => void copyText(message.content)}>
            <Copy size={15} aria-hidden="true" />
          </button>
          <button type="button" aria-label="编辑消息" title="编辑消息" onClick={() => onEditMessage(message)}>
            <Pencil size={15} aria-hidden="true" />
          </button>
        </div>
      </div>
    );
  }

  const confirmations = response?.pending_confirmations ?? [];

  return (
    <article className="suna-chat-assistant-turn" aria-label="Suna" data-message-id={message.id}>
      <div className="suna-chat-answer ai-markdown-body">
        <LazyAIAnswerRenderer answer={message.content} literatureResults={[]} />
      </div>
      {response && (response.context_status.memory_count > 0 || response.context_status.skill_count > 0 || response.context_status.tool_count > 0) && (
        <div className="suna-chat-inline-status" aria-label={response.context_status ? "本轮上下文" : undefined}>
          <div className="suna-chat-inline-status-line">
            {response.context_status.memory_count > 0 && <span>{response.context_status.memory_count} 条记忆</span>}
            {response.context_status.skill_count > 0 && <span>{response.context_status.skill_count} 个技能</span>}
            {response.context_status.tool_count > 0 && <span>{response.context_status.tool_count} 个工具</span>}
          </div>
        </div>
      )}
      {evidence && <CitationPanel auditId={evidence.auditId} evidence={evidence.evidence} />}
      {response?.follow_up_questions.length ? (
        <div className="suna-chat-inline-status" aria-label="需要补充的信息">
          <strong>需要补充的信息</strong>
          <div className="suna-chat-permission-actions">
            {response.follow_up_questions.map((question) => (
              <Button variant="actionSecondary" type="button" key={question} onClick={() => onFollowUp(question)}>{question}</Button>
            ))}
          </div>
        </div>
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
      {!loading && <WebFeedback messageId={message.id} />}
      <div className="suna-chat-message-actions">
        <button type="button" aria-label="复制回答" title="复制回答" onClick={() => void copyText(message.content)}>
          <Copy size={15} aria-hidden="true" />
        </button>
      </div>
    </article>
  );
});

export default NativeChatMessage;
