import type { Message, MessagePage } from "../../bridge/desktop";

export function messagePage(messages: Message[], hasOlder = false): MessagePage {
  const oldest = messages[0];
  return {
    messages,
    has_older: hasOlder,
    oldest_rowid: oldest ? 1 : null,
    oldest_created_at: oldest?.created_at ?? null,
  };
}
