import type { Message } from "../../bridge/desktop";
import type { MessagePage } from "../../bridge/desktop";

export const CHAT_MESSAGE_PAGE_SIZE = 80;

export type OlderMessagesPaginationGuard = {
  conversationId: string;
  epoch: number;
  owner: number;
};

export function shouldApplyOlderMessagesScrollRestore(
  guard: OlderMessagesPaginationGuard,
  current: { conversationId: string | null; epoch: number; owner: number },
) {
  return (
    guard.conversationId === current.conversationId
    && guard.epoch === current.epoch
    && guard.owner === current.owner
  );
}

export const TERMINAL_AGENT_RUN_STATES = new Set([
  "completed",
  "cancelled",
  "failed",
  "interrupted",
]);

export function mergeTailMessages(current: Message[], page: MessagePage): Message[] {
  if (!page.messages.length) return current;
  const tailIds = new Set(page.messages.map((message) => message.id));
  const prefix = current.filter((message) => !tailIds.has(message.id));
  return [...prefix, ...page.messages];
}

export function prependOlderMessages(current: Message[], older: Message[]): Message[] {
  if (!older.length) return current;
  const existingIds = new Set(current.map((message) => message.id));
  const uniqueOlder = older.filter((message) => !existingIds.has(message.id));
  if (!uniqueOlder.length) return current;
  return [...uniqueOlder, ...current];
}
