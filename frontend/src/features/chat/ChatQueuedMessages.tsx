import { X } from "lucide-react";

export default function ChatQueuedMessages({
  messages,
  onRemove,
}: {
  messages: string[];
  onRemove: (index: number) => void;
}) {
  if (!messages.length) return null;
  return (
    <div className="suna-chat-queued" aria-label="排队中的消息">
      <strong>排队中 · {messages.length}</strong>
      <ul>
        {messages.map((message, index) => (
          <li key={`${message}-${index}`}>
            <span>{message}</span>
            <button type="button" aria-label="移除排队消息" onClick={() => onRemove(index)}>
              <X size={14} aria-hidden="true" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
