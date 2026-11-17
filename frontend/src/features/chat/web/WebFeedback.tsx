import { useEffect, useState } from "react";
import { ThumbsDown, ThumbsUp } from "lucide-react";
import { desktop, isDesktopRuntime } from "../../../bridge/desktop";

type Rating = "up" | "down";

/** Align reason codes with steel-agent AgentFeedback. */
const DOWN_REASONS = [
  { value: "intent_error", label: "理解错问题" },
  { value: "not_answered", label: "没回答到点上" },
  { value: "incorrect", label: "内容有误" },
  { value: "unsupported", label: "依据不相关" },
  { value: "incomplete", label: "任务没完成" },
  { value: "impractical", label: "方案不实用" },
  { value: "verbose", label: "太啰嗦/格式差" },
  { value: "other", label: "其他" },
] as const;

export default function WebFeedback({
  messageId,
  conversationId,
  onFeedback,
}: {
  messageId: string;
  conversationId?: string;
  onFeedback: (rating: Rating, reason?: string) => Promise<boolean>;
}) {
  const [rating, setRating] = useState<Rating | null>(null);
  const [showReason, setShowReason] = useState(false);
  const [reason, setReason] = useState("");
  const [sending, setSending] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let active = true;
    setRating(null);
    setShowReason(false);
    setReason("");
    setFailed(false);
    if (!conversationId || !isDesktopRuntime()) return () => { active = false; };
    void desktop.getAgentFeedback(conversationId, messageId).then((row) => {
      if (!active || !row) return;
      if (row.rating === "up" || row.rating === "down") setRating(row.rating);
    }).catch(() => undefined);
    return () => { active = false; };
  }, [conversationId, messageId]);

  const submit = async (next: Rating, nextReason?: string) => {
    if (sending) return;
    setSending(true);
    setFailed(false);
    let ok = false;
    try {
      ok = (await onFeedback(next, nextReason)) === true;
    } catch {
      ok = false;
    }
    setSending(false);
    if (ok) {
      setRating(next);
      setShowReason(false);
      setReason("");
    } else {
      setFailed(true);
    }
  };

  const handleUp = () => {
    void submit("up");
  };

  const handleDown = () => {
    if (rating === "down") {
      setShowReason((value) => !value);
      return;
    }
    setShowReason(true);
    setReason("");
  };

  const submitReason = () => {
    void submit("down", reason || "other");
  };

  const downActive = rating === "down" || showReason;

  return (
    <div className="flex flex-wrap items-center gap-1">
      <button
        type="button"
        onClick={handleUp}
        disabled={sending}
        aria-label="有用"
        aria-pressed={rating === "up"}
        title="有用"
        className={`flex h-7 w-7 items-center justify-center rounded-md transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${
          rating === "up"
            ? "bg-[var(--suna-text)] text-[var(--suna-bg)]"
            : "text-[var(--suna-text-muted)] hover:bg-[var(--suna-bg-hover)] hover:text-[var(--suna-text)]"
        }`}
      >
        <ThumbsUp size={15} />
      </button>
      <button
        type="button"
        onClick={handleDown}
        disabled={sending}
        aria-label="无用"
        aria-pressed={rating === "down"}
        title="无用"
        className={`flex h-7 w-7 items-center justify-center rounded-md transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${
          downActive
            ? "bg-[var(--suna-text)] text-[var(--suna-bg)]"
            : "text-[var(--suna-text-muted)] hover:bg-[var(--suna-bg-hover)] hover:text-[var(--suna-text)]"
        }`}
      >
        <ThumbsDown size={15} />
      </button>
      {showReason && (
        <div className="ml-1 flex flex-wrap items-center gap-1.5">
          <span className="text-xs text-[var(--suna-text-muted)]">原因：</span>
          {DOWN_REASONS.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => setReason(opt.value)}
              disabled={sending}
              className={`rounded-full border px-2.5 py-0.5 text-xs transition-colors disabled:cursor-not-allowed disabled:opacity-50 ${
                reason === opt.value
                  ? "border-[var(--suna-text)] bg-[var(--suna-text)] text-[var(--suna-bg)] font-medium"
                  : "border-[var(--suna-line)] bg-[var(--suna-bg-raised)] text-[var(--suna-text-soft)] hover:bg-[var(--suna-bg-hover)]"
              }`}
            >
              {opt.label}
            </button>
          ))}
          <button
            type="button"
            onClick={submitReason}
            disabled={sending}
            className="rounded-full bg-[var(--suna-text)] px-2.5 py-0.5 text-xs font-medium text-[var(--suna-bg)] disabled:cursor-not-allowed disabled:opacity-40"
          >
            {sending ? "提交中…" : "提交"}
          </button>
        </div>
      )}
      {failed && (
        <span role="status" className="text-xs text-[var(--suna-danger,#b42318)]">
          提交失败，请重试
        </span>
      )}
      <span data-feedback-message-id={messageId} className="hidden" />
    </div>
  );
}
