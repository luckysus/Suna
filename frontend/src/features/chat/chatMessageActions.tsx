import { createContext, useContext, useMemo, useRef, type ReactNode } from "react";
import type { Message } from "../../bridge/desktop";
import type { PermissionDecision } from "../../bridge/generated/protocol";

export type ChatMessageActions = {
  onEditMessage: (message: Message) => void;
  onFollowUp: (question: string) => void;
  onResolvePermission: (permissionId: string, decision: PermissionDecision) => void;
  onForkMessage?: (message: Message) => void;
  onFeedback?: (message: Message, rating: "up" | "down", reason?: string) => Promise<boolean>;
};

const ChatMessageActionsContext = createContext<ChatMessageActions | null>(null);

export function ChatMessageActionsProvider({
  actions,
  children,
}: {
  actions: ChatMessageActions;
  children: ReactNode;
}) {
  const ref = useRef(actions);
  ref.current = actions;
  const stable = useMemo<ChatMessageActions>(
    () => ({
      onEditMessage: (message) => ref.current.onEditMessage(message),
      onFollowUp: (question) => ref.current.onFollowUp(question),
      onResolvePermission: (permissionId, decision) => ref.current.onResolvePermission(permissionId, decision),
      onForkMessage: (message) => ref.current.onForkMessage?.(message),
      onFeedback: (message, rating, reason) =>
        ref.current.onFeedback?.(message, rating, reason) ?? Promise.resolve(false),
    }),
    [],
  );
  return <ChatMessageActionsContext.Provider value={stable}>{children}</ChatMessageActionsContext.Provider>;
}

export function useChatMessageActions() {
  const value = useContext(ChatMessageActionsContext);
  if (!value) throw new Error("ChatMessageActionsProvider is required");
  return value;
}
