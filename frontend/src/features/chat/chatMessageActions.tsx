import { createContext, useContext, useMemo, useRef, type ReactNode } from "react";
import type { Message } from "../../bridge/desktop";
import type { PermissionDecision } from "../../bridge/generated/protocol";

export type ChatMessageActions = {
  onEditMessage: (message: Message) => void;
  onFollowUp: (question: string) => void;
  onResolvePermission: (permissionId: string, decision: PermissionDecision) => void;
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
