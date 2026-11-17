import { useEffect, useState } from "react";
import { formatRunDurationParts } from "./runDuration";

/** Live / settled turn timing — mirrors steel-agent AgentTurnTimer. */
export default function ChatTurnTimer({
  active,
  waiting = false,
  startedAt,
  endedAt = null,
}: {
  active: boolean;
  waiting?: boolean;
  startedAt: number | null;
  endedAt?: number | null;
}) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (!active || !startedAt) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [active, startedAt]);

  const end = active ? now : endedAt;
  const hasTiming = Boolean(startedAt && end && end >= startedAt);
  if (!hasTiming && !waiting) return null;

  const duration = hasTiming && startedAt && end
    ? formatRunDurationParts(end - startedAt)
    : null;
  const label = active ? "正在处理" : waiting ? "等待回答" : "用时";

  return (
    <div className="suna-chat-turn-timer" data-agent-turn-timing aria-live="polite">
      <span>{label}{duration ? " " : ""}</span>
      {duration ? (
        <span className="suna-chat-turn-timer-parts" aria-label={`${label} ${duration.label}`}>
          {duration.parts.map((part) => (
            <span key={part.label} className="suna-chat-turn-timer-part">
              <span
                className="suna-chat-turn-timer-value"
                style={{ width: `${part.valueWidthCh}ch` }}
                aria-hidden="true"
              >
                {part.value}
              </span>
              <span aria-hidden="true">{part.label}</span>
            </span>
          ))}
        </span>
      ) : null}
    </div>
  );
}
