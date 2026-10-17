import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { desktop, type PostgresIngestionJob, type SourceDocumentRecord } from "../../bridge/desktop";
import KnowledgeCenterWorkspace from "./KnowledgeCenterWorkspace";

vi.mock("../../bridge/desktop", () => ({
  desktop: {
    listKnowledgeBases: vi.fn(),
    getKnowledgeBaseMetrics: vi.fn(),
    getKnowledgeHealth: vi.fn(),
    listKnowledgeDocuments: vi.fn(),
    listPostgresIngestionJobs: vi.fn(),
    listPostgresKnowledgeChunks: vi.fn(),
    listPostgresWikiPages: vi.fn(),
    listPostgresKnowledgeEdges: vi.fn(),
    listPostgresTags: vi.fn(),
    listenSchedulerProgress: vi.fn(),
    getSetting: vi.fn(),
    createKnowledgeBase: vi.fn(),
    importPostgresDocument: vi.fn(),
    importPostgresDirectory: vi.fn(),
  },
}));

vi.mock("../../hooks/useNativeFileDrop", () => ({
  useNativeFileDrop: () => undefined,
}));

const now = "2026-10-17T00:00:00Z";
const baseA = {
  id: "kb-a",
  name: "知识库 A",
  description: "A",
  tags: [] as string[],
  library_type: "research",
  visibility: "private" as const,
  created_at: now,
  updated_at: now,
};
const baseB = {
  id: "kb-b",
  name: "知识库 B",
  description: "B",
  tags: [] as string[],
  library_type: "research",
  visibility: "private" as const,
  created_at: now,
  updated_at: now,
};

function job(id: string, knowledgeBaseId: string, state: string): PostgresIngestionJob {
  return {
    id,
    knowledge_base_id: knowledgeBaseId,
    state,
    progress: state === "completed" ? 100 : 40,
    attempts: 1,
    created_at: now,
    updated_at: now,
  } as PostgresIngestionJob;
}

function document(id: string, knowledgeBaseId: string, name: string): SourceDocumentRecord {
  return {
    id,
    knowledge_base_id: knowledgeBaseId,
    display_name: name,
    source_kind: "pdf",
    file_size: 1000,
    active_version_id: "version-1",
    created_at: now,
    updated_at: now,
  } as SourceDocumentRecord;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((ok, err) => {
    resolve = ok;
    reject = err;
  });
  return { promise, resolve, reject };
}

describe("KnowledgeCenterWorkspace", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.mocked(desktop.listKnowledgeBases).mockResolvedValue([baseA, baseB]);
    vi.mocked(desktop.getKnowledgeBaseMetrics).mockResolvedValue([
      { knowledge_base_id: "kb-a", document_count: 0, indexed_document_count: 0, chunk_count: 0, embedded_chunk_count: 0 },
      { knowledge_base_id: "kb-b", document_count: 0, indexed_document_count: 0, chunk_count: 0, embedded_chunk_count: 0 },
    ]);
    vi.mocked(desktop.getKnowledgeHealth).mockResolvedValue({
      knowledge_base_count: 2,
      document_count: 0,
      chunk_count: 0,
      indexed_chunk_count: 0,
      retrieval_count: 0,
    } as never);
    vi.mocked(desktop.listPostgresTags).mockResolvedValue([]);
    vi.mocked(desktop.listPostgresKnowledgeChunks).mockResolvedValue([]);
    vi.mocked(desktop.listenSchedulerProgress).mockResolvedValue(() => undefined);
    vi.mocked(desktop.getSetting).mockResolvedValue(null);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("isolates pending refresh when switching A → B after A document refresh fails", async () => {
    let aState: "embedding" | "completed" = "embedding";
    const aDocFail = deferred<SourceDocumentRecord[]>();
    const ipc = { aJobs: 0, bJobs: 0, aDocs: 0, bDocs: 0 };

    vi.mocked(desktop.listPostgresIngestionJobs).mockImplementation(async (id: string) => {
      if (id === "kb-a") {
        ipc.aJobs += 1;
        return [job("job-a", "kb-a", aState)];
      }
      ipc.bJobs += 1;
      return [];
    });
    vi.mocked(desktop.listKnowledgeDocuments).mockImplementation(async (id: string) => {
      if (id === "kb-a") {
        ipc.aDocs += 1;
        if (aState === "completed" && ipc.aDocs > 1) return aDocFail.promise;
        return [];
      }
      ipc.bDocs += 1;
      return [document("doc-b", "kb-b", "B.pdf")];
    });

    render(<KnowledgeCenterWorkspace />);
    expect(await screen.findByRole("button", { name: /知识库 A/ })).toBeInTheDocument();
    await waitFor(() => expect(ipc.aJobs).toBeGreaterThan(0));

    aState = "completed";
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2600);
    });
    await waitFor(() => expect(ipc.aDocs).toBeGreaterThan(1));

    const aDocsBeforeSwitch = ipc.aDocs;
    const aJobsBeforeSwitch = ipc.aJobs;
    fireEvent.click(screen.getByRole("button", { name: /知识库 B/ }));
    expect(await screen.findByText("B.pdf")).toBeInTheDocument();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });

    expect(ipc.aDocs).toBe(aDocsBeforeSwitch);
    expect(ipc.aJobs).toBe(aJobsBeforeSwitch);
    expect(ipc.bJobs).toBeGreaterThan(0);
    expect(ipc.bDocs).toBeGreaterThan(0);

    await act(async () => {
      aDocFail.reject(new Error("stale A refresh"));
    });
  });

  it("retries document refresh after completion until success, then stops", async () => {
    let aState: "embedding" | "completed" = "embedding";
    let failOnce = true;
    const ipc = { docs: 0, jobs: 0 };

    vi.mocked(desktop.listPostgresIngestionJobs).mockImplementation(async (id: string) => {
      if (id !== "kb-a") return [];
      ipc.jobs += 1;
      return [job("job-a", "kb-a", aState)];
    });
    vi.mocked(desktop.listKnowledgeDocuments).mockImplementation(async (id: string) => {
      if (id !== "kb-a") return [];
      ipc.docs += 1;
      if (aState === "completed" && failOnce && ipc.docs > 1) {
        failOnce = false;
        throw new Error("temporary document refresh failure");
      }
      if (aState === "completed") return [document("doc-a", "kb-a", "完成文档.pdf")];
      return [];
    });

    render(<KnowledgeCenterWorkspace />);
    await screen.findByRole("button", { name: /知识库 A/ });
    aState = "completed";
    await act(async () => {
      await vi.advanceTimersByTimeAsync(2600);
    });
    await waitFor(() => expect(screen.getByText("完成文档.pdf")).toBeInTheDocument());
    const docsAfterSuccess = ipc.docs;
    await act(async () => {
      await vi.advanceTimersByTimeAsync(8000);
    });
    expect(ipc.docs).toBe(docsAfterSuccess);
  });

  it("records IPC counts across A/B selection without stale document pulls", async () => {
    const counts: Record<string, number> = {};
    const bump = (key: string) => {
      counts[key] = (counts[key] || 0) + 1;
    };
    vi.mocked(desktop.listPostgresIngestionJobs).mockImplementation(async (id: string) => {
      bump(`jobs:${id}`);
      return [];
    });
    vi.mocked(desktop.listKnowledgeDocuments).mockImplementation(async (id: string) => {
      bump(`docs:${id}`);
      return [];
    });
    vi.mocked(desktop.getKnowledgeBaseMetrics).mockImplementation(async () => {
      bump("metrics");
      return [
        { knowledge_base_id: "kb-a", document_count: 0, indexed_document_count: 0, chunk_count: 0, embedded_chunk_count: 0 },
        { knowledge_base_id: "kb-b", document_count: 0, indexed_document_count: 0, chunk_count: 0, embedded_chunk_count: 0 },
      ];
    });

    render(<KnowledgeCenterWorkspace />);
    await screen.findByRole("button", { name: /知识库 A/ });
    fireEvent.click(screen.getByRole("button", { name: /知识库 B/ }));
    await waitFor(() => expect(counts["docs:kb-b"]).toBeGreaterThan(0));
    fireEvent.click(screen.getByRole("button", { name: /知识库 A/ }));
    await waitFor(() => expect(counts["docs:kb-a"]).toBeGreaterThan(1));
    expect(counts["jobs:kb-a"]).toBeGreaterThan(0);
    expect(counts["jobs:kb-b"]).toBeGreaterThan(0);
    expect(counts.metrics).toBeGreaterThan(0);
  });
});
