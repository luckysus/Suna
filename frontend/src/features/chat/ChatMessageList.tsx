import { useEffect, useRef, type RefObject } from "react";
import { flushSync } from "react-dom";
import { Button } from "../../components/ui/button";
import { useChatStore } from "../../stores/chatStore";
import { ChatMessageActionsProvider } from "./chatMessageActions";
import NativeChatMessage from "./NativeChatMessage";
import type { ChatMessageActions } from "./chatMessageActions";
import type { OlderMessagesPaginationGuard } from "./chatMessagePage";

function waitForPaint() {
  return new Promise<void>((resolve) => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => resolve());
    });
  });
}

export default function ChatMessageList({
  loading,
  messageActions,
  onLoadOlderMessages,
  isOlderMessagesPaginationActive,
  scrollContainerRef,
}: {
  loading: boolean;
  messageActions: ChatMessageActions;
  onLoadOlderMessages: () => Promise<{ loaded: boolean; guard: OlderMessagesPaginationGuard | null }>;
  isOlderMessagesPaginationActive: (guard: OlderMessagesPaginationGuard) => boolean;
  scrollContainerRef?: RefObject<HTMLDivElement | null>;
}) {
  const messages = useChatStore((state) => state.messages);
  const hasOlderMessages = useChatStore((state) => state.hasOlderMessages);
  const loadingOlderMessages = useChatStore((state) => state.loadingOlderMessages);
  const loadingOlderRef = useRef(false);

  useEffect(() => {
    loadingOlderRef.current = false;
  }, [messages.length, hasOlderMessages]);

  const handleLoadOlder = async () => {
    if (loadingOlderRef.current || loadingOlderMessages) return;
    loadingOlderRef.current = true;
    const container = scrollContainerRef?.current;
    const previousScrollHeight = container?.scrollHeight ?? 0;
    const previousScrollTop = container?.scrollTop ?? 0;
    try {
      const { loaded, guard } = await onLoadOlderMessages();
      if (!loaded || !guard || !container || !isOlderMessagesPaginationActive(guard)) return;
      flushSync(() => undefined);
      await waitForPaint();
      if (!isOlderMessagesPaginationActive(guard)) return;
      const delta = container.scrollHeight - previousScrollHeight;
      container.scrollTop = previousScrollTop + delta;
    } finally {
      loadingOlderRef.current = false;
    }
  };

  return (
    <ChatMessageActionsProvider actions={messageActions}>
      {hasOlderMessages && (
        <div className="suna-chat-load-older">
          <Button variant="ghost" type="button" disabled={loadingOlderMessages} onClick={() => void handleLoadOlder()}>
            {loadingOlderMessages ? "加载更早消息…" : "加载更早消息"}
          </Button>
        </div>
      )}
      {messages.map((message, index) => (
        <NativeChatMessage
          key={message.id}
          message={message}
          index={index}
          loading={loading}
        />
      ))}
    </ChatMessageActionsProvider>
  );
}
