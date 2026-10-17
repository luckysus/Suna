import { describe, expect, it } from "vitest";
import type { Message } from "../../bridge/desktop";
import { mergeTailMessages, prependOlderMessages, shouldApplyOlderMessagesScrollRestore } from "./chatMessagePage";

function message(id: string, content: string): Message {
  return {
    id,
    conversation_id: "c1",
    role: "user",
    content,
    response_json: null,
    created_at: `2026-01-01T00:00:0${id.slice(-1)}Z`,
  };
}

describe("chatMessagePage", () => {
  it("prepends older messages without duplicating ids", () => {
    const current = [message("3", "c"), message("4", "d")];
    const older = [message("1", "a"), message("2", "b"), message("3", "c")];
    expect(prependOlderMessages(current, older).map((item) => item.id)).toEqual(["1", "2", "3", "4"]);
  });

  it("shouldApplyOlderMessagesScrollRestore requires matching conversation, epoch, and owner", () => {
    const guard = { conversationId: "c1", epoch: 2, owner: 5 };
    expect(shouldApplyOlderMessagesScrollRestore(guard, { conversationId: "c1", epoch: 2, owner: 5 })).toBe(true);
    expect(shouldApplyOlderMessagesScrollRestore(guard, { conversationId: "c2", epoch: 2, owner: 5 })).toBe(false);
    expect(shouldApplyOlderMessagesScrollRestore(guard, { conversationId: "c1", epoch: 3, owner: 5 })).toBe(false);
    expect(shouldApplyOlderMessagesScrollRestore(guard, { conversationId: "c1", epoch: 2, owner: 6 })).toBe(false);
  });

  it("mergeTailMessages keeps historical prefix when refreshing the latest page", () => {
    const current = [message("1", "a"), message("2", "b"), message("3", "c")];
    const page = {
      messages: [message("3", "c-updated"), message("4", "d")],
      has_older: true,
      oldest_rowid: 99,
      oldest_created_at: "2026-01-01T00:00:09Z",
    };
    const merged = mergeTailMessages(current, page);
    expect(merged.map((item) => item.id)).toEqual(["1", "2", "3", "4"]);
    expect(merged.find((item) => item.id === "3")?.content).toBe("c-updated");
  });
});
