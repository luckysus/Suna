import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SunaApp from "./SunaApp";
import { desktop, type Conversation } from "../bridge/desktop";
import { resetAgentStoreForTests } from "../stores/agentStore";
import { resetChatStoreForTests } from "../stores/chatStore";
import { resetKnowledgeStoreForTests } from "../stores/knowledgeStore";
import { resetModelStoreForTests } from "../stores/modelStore";

vi.mock("../bridge/desktop", () => ({
  isDesktopRuntime: () => false,
  desktop: {
    listConversations: vi.fn(),
    searchHistory: vi.fn(),
    createConversation: vi.fn(),
    updateConversationTitle: vi.fn(),
    updateConversationPinned: vi.fn(),
    archiveConversation: vi.fn(),
    restoreConversation: vi.fn(),
    listArchivedConversations: vi.fn().mockResolvedValue([]),
    deleteConversationLocal: vi.fn(),
    listMessages: vi.fn(),
    getConversationDraft: vi.fn(),
    saveConversationDraft: vi.fn(),
    listenAgentEvents: vi.fn(),
    replayAgentRun: vi.fn(),
    desktopAgentChat: vi.fn(),
    resolveAgentPermission: vi.fn(),
    cancelAgentRun: vi.fn(),
    steerAgentRun: vi.fn(),
    followUpAgentRun: vi.fn(),
    recoverAgentRuns: vi.fn(),
    listKnowledgeBases: vi.fn(),
    queryLocalKnowledge: vi.fn(),
    listProviderProfiles: vi.fn(),
    listAgentProfiles: vi.fn(),
    setDefaultProvider: vi.fn(),
    resolveKnowledgeCitation: vi.fn(),
    openFileDialog: vi.fn(),
    saveFileDialog: vi.fn(),
    exportConversation: vi.fn(),
    getSetting: vi.fn().mockResolvedValue(null),
    setSetting: vi.fn().mockResolvedValue(undefined),
    listSkills: vi.fn().mockResolvedValue({ skills: [], errors: [] }),
    listMcpServers: vi.fn().mockResolvedValue([]),
    listResearchProjects: vi.fn().mockResolvedValue([]),
    getCurrentResearchProject: vi.fn().mockResolvedValue(null),
  },
}));

const conversation: Conversation = {
  id: "conversation-1",
  title: "Q355B 标准",
  created_at: "2026-08-05T10:00:00Z",
  updated_at: "2026-08-05T10:00:00Z",
  pinned: false,
  archived: false,
};

describe("SunaApp global sidebar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetChatStoreForTests();
    resetAgentStoreForTests();
    resetModelStoreForTests();
    resetKnowledgeStoreForTests();
    vi.mocked(desktop.listConversations).mockResolvedValue([conversation]);
    vi.mocked(desktop.searchHistory).mockResolvedValue([]);
    vi.mocked(desktop.updateConversationTitle).mockResolvedValue(undefined);
    vi.mocked(desktop.updateConversationPinned).mockResolvedValue(undefined);
    vi.mocked(desktop.archiveConversation).mockResolvedValue(undefined);
    vi.mocked(desktop.deleteConversationLocal).mockResolvedValue(undefined);
    vi.mocked(desktop.listMessages).mockResolvedValue([]);
    vi.mocked(desktop.getConversationDraft).mockResolvedValue("");
    vi.mocked(desktop.saveConversationDraft).mockResolvedValue(undefined);
    vi.mocked(desktop.listenAgentEvents).mockResolvedValue(() => undefined);
    vi.mocked(desktop.replayAgentRun).mockResolvedValue([]);
    vi.mocked(desktop.listKnowledgeBases).mockResolvedValue([]);
    vi.mocked(desktop.listProviderProfiles).mockResolvedValue([]);
    vi.mocked(desktop.listAgentProfiles).mockResolvedValue([]);
    vi.mocked(desktop.openFileDialog).mockResolvedValue(null);
    vi.mocked(desktop.recoverAgentRuns).mockResolvedValue([]);
  });

  it("renames and pins the conversation from the global sidebar", async () => {
    vi.spyOn(window, "prompt").mockReturnValue("Q355B 新标题");
    render(<SunaApp />);

    const rename = await screen.findByRole("button", { name: "重命名" });
    fireEvent.click(rename);
    await waitFor(() => expect(desktop.updateConversationTitle).toHaveBeenCalledWith(conversation.id, "Q355B 新标题"));

    fireEvent.click(screen.getByRole("button", { name: "置顶对话" }));
    await waitFor(() => expect(desktop.updateConversationPinned).toHaveBeenCalledWith(conversation.id, true));
  });
});
