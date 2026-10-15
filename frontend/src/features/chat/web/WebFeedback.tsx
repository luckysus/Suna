import { useState } from "react";
import { ThumbsDown, ThumbsUp } from "lucide-react";

type Rating = "up" | "down";

const REASONS = [
  ["intent_error", "意图识别错误"],
  ["evidence_irrelevant", "证据不相关"],
  ["plan_impractical", "方案不实用"],
  ["other", "其他"],
] as const;

export default function WebFeedback({
  messageId,
  onFeedback,
}: {
  messageId: string;
  onFeedback?: (rating: Rating, reason?: string) => void;
}) {
  const [rating, setRating] = useState<Rating | null>(null);
  const [reason, setReason] = useState("");
  const [showReason, setShowReason] = useState(false);
  const send = (next: Rating) => {
    setRating(next);
    setShowReason(next === "down");
    onFeedback?.(next);
  };
  return (
    <div className="mt-2 flex flex-wrap items-center gap-1">
      <button type="button" onClick={() => send("up")} aria-label="有用" title="有用" className={`flex h-7 w-7 items-center justify-center rounded-md transition-colors ${rating === "up" ? "bg-[var(--suna-text)] text-[var(--suna-bg)]" : "text-[var(--suna-text-muted)] hover:bg-[var(--suna-bg-hover)] hover:text-[var(--suna-text)]"}`}>
        <ThumbsUp size={15} />
      </button>
      <button type="button" onClick={() => send("down")} aria-label="无用" title="无用" className={`flex h-7 w-7 items-center justify-center rounded-md transition-colors ${rating === "down" ? "bg-[var(--suna-text)] text-[var(--suna-bg)]" : "text-[var(--suna-text-muted)] hover:bg-[var(--suna-bg-hover)] hover:text-[var(--suna-text)]"}`}>
        <ThumbsDown size={15} />
      </button>
      {showReason && (
        <div className="ml-1 flex flex-wrap items-center gap-1.5">
          <span className="text-xs text-[var(--suna-text-muted)]">原因：</span>
          {REASONS.map(([value, label]) => (
            <button key={value} type="button" onClick={() => setReason(value)} className={`rounded-full border px-2.5 py-0.5 text-xs transition-colors ${reason === value ? "border-[var(--suna-text)] bg-[var(--suna-text)] text-[var(--suna-bg)] font-medium" : "border-[var(--suna-line)] bg-[var(--suna-bg-raised)] text-[var(--suna-text-soft)] hover:bg-[var(--suna-bg-hover)]"}`}>
              {label}
            </button>
          ))}
          <button type="button" onClick={() => { onFeedback?.("down", reason || "other"); setShowReason(false); }} disabled={!reason} className="rounded-full bg-[var(--suna-text)] px-2.5 py-0.5 text-xs font-medium text-[var(--suna-bg)] disabled:opacity-40">
            提交
          </button>
        </div>
      )}
      <span data-feedback-message-id={messageId} className="hidden" />
    </div>
  );
}
