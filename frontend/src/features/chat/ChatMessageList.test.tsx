import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useEffect } from "react";
import ChatMessageList from "./ChatMessageList";
import { resetChatStoreForTests, useChatStore } from "../../stores/chatStore";
import type { OlderMessagesPaginationGuard } from "./chatMessagePage";

vi.mock("./NativeChatMessage", () => ({
  default: ({ message }: { message: { id: string; content: string } }) => (
    <div data-message-id={message.id}>{message.content}</div>
  ),
}));

function SeedMessages({ count }: { count: number }) {
  useEffect(() => {
    useChatStore.getState().setMessages(
      Array.from({ length: count }, (_, index) => ({
        id: `message-${index}`,
        conversation_id: "c1",
        role: index % 2 ? "assistant" : "user",
        content: `body-${index}`,
        response_json: null,
        created_at: "2026-10-17T00:00:00Z",
      })),
    );
    useChatStore.getState().setHasOlderMessages(true);
    useChatStore.getState().setLoadingOlderMessages(false);
  }, [count]);
  return null;
}

describe("ChatMessageList scroll ownership", () => {
  beforeEach(() => {
    resetChatStoreForTests();
    vi.stubGlobal(
      "requestAnimationFrame",
      (cb: FrameRequestCallback) => window.setTimeout(() => cb(performance.now()), 0) as unknown as number,
    );
  });

  it("does not restore scroll when pagination guard becomes inactive during rAF wait", async () => {
    const container = document.createElement("div");
    Object.defineProperty(container, "scrollHeight", { configurable: true, get: () => 1000 });
    Object.defineProperty(container, "scrollTop", {
      configurable: true,
      get: () => 200,
      set: vi.fn(),
    });
    const scrollContainerRef = { current: container };
    let active = true;
    const guard: OlderMessagesPaginationGuard = { conversationId: "c-a", epoch: 1, owner: 1 };
    const onLoadOlderMessages = vi.fn(async () => {
      await new Promise((resolve) => setTimeout(resolve, 30));
      return { loaded: true, guard };
    });
    const isOlderMessagesPaginationActive = vi.fn(() => active);

    render(
      <>
        <SeedMessages count={4} />
        <ChatMessageList
          loading={false}
          messageActions={{} as never}
          scrollContainerRef={scrollContainerRef}
          onLoadOlderMessages={onLoadOlderMessages}
          isOlderMessagesPaginationActive={isOlderMessagesPaginationActive}
        />
      </>,
    );

    fireEvent.click(await screen.findByRole("button", { name: "加载更早消息" }));
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
      active = false;
      await new Promise((resolve) => setTimeout(resolve, 40));
    });
    await waitFor(() => expect(onLoadOlderMessages).toHaveBeenCalled());
    const setter = Object.getOwnPropertyDescriptor(container, "scrollTop")?.set;
    expect(setter).not.toHaveBeenCalled();
  });
});
