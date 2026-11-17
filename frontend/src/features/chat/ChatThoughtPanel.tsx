import { useEffect, useRef, useState } from "react";
import { BrainCircuit, ChevronDown, ChevronRight } from "lucide-react";
import { useLocale } from "../../i18n/locale";

function formatThoughtSeconds(ms: number | null | undefined) {
  if (ms == null || ms < 0) return null;
  return String(Math.max(1, Math.round(ms / 1000)));
}

/** Collapsible model reasoning — mirrors steel-agent ThoughtStep. */
export default function ChatThoughtPanel({
  reasoning,
  reasoningMs = null,
  streaming = false,
}: {
  reasoning: string;
  reasoningMs?: number | null;
  streaming?: boolean;
}) {
  const { t } = useLocale();
  const text = reasoning.trim();
  const [open, setOpen] = useState(() => streaming);
  const wasStreaming = useRef(streaming);

  useEffect(() => {
    // Only auto-collapse when thinking finishes. Never force-open again —
    // that caused expand/collapse flicker when status briefly flipped.
    if (wasStreaming.current && !streaming) setOpen(false);
    wasStreaming.current = streaming;
  }, [streaming]);

  if (!text) return null;

  const seconds = streaming ? null : formatThoughtSeconds(reasoningMs ?? 0);
  const label = streaming
    ? t("thoughtThinking")
    : seconds
      ? t("thoughtDoneWithDuration", { seconds })
      : t("thoughtDone");

  return (
    <div className="suna-chat-thought" data-thought-streaming={streaming ? "true" : "false"}>
      <button
        type="button"
        className="suna-chat-thought-toggle"
        aria-expanded={open}
        aria-label={open ? t("thoughtCollapse") : t("thoughtExpand")}
        onClick={() => setOpen((value) => !value)}
      >
        <BrainCircuit size={14} aria-hidden="true" />
        <span>{label}</span>
        {open ? <ChevronDown size={14} aria-hidden="true" /> : <ChevronRight size={14} aria-hidden="true" />}
      </button>
      {open ? <div className="suna-chat-thought-body">{text}</div> : null}
    </div>
  );
}
