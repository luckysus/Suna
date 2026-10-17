import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { desktop, type SteelModelSummary } from "../../bridge/desktop";
import ModelRegistryPanel, { modelCardMetrics, modelCardTitle } from "./ModelRegistryPanel";

vi.mock("../../bridge/desktop", () => ({
  desktop: {
    listAllSteelModels: vi.fn(),
    setActiveSteelModel: vi.fn(),
    deleteSteelModel: vi.fn(),
    getSteelModelScientificCards: vi.fn(),
  },
}));

const model = (overrides: Partial<SteelModelSummary> = {}): SteelModelSummary => ({
  id: "model-1",
  lineage_id: "sklearn:dataset-1",
  kind: "sklearn_artifact",
  version: 2,
  source_task_id: "018f3c2a-0000-0000-0000-000000000000",
  model_sha256: "a".repeat(64),
  model_type: "xgboost",
  metrics_json: JSON.stringify({
    validation: { sample_count: 20, mae: 1.5, rmse: 2.5, r2: 0.97 },
    train: { sample_count: 80, mae: 0.5, rmse: 1.0, r2: 0.99 },
  }),
  has_artifact_json: true,
  has_model_base64: false,
  is_active: false,
  created_at: "2026-10-10T08:30:00Z",
  ...overrides,
});

const page = (items: SteelModelSummary[]) => ({ items, total: items.length, has_more: false });

const datasets = [
  { id: "dataset-1", sourceName: "Q690 heats.csv" },
] as unknown as Parameters<typeof ModelRegistryPanel>[0]["datasets"];

function renderPanel() {
  return render(<ModelRegistryPanel datasets={datasets} onNotice={vi.fn()} onError={vi.fn()} />);
}

beforeEach(() => {
  vi.clearAllMocks();
  vi.mocked(desktop.listAllSteelModels).mockResolvedValue(page([]));
});

describe("ModelRegistryPanel", () => {
  it("renders model cards with dataset, task, version and validation metrics", async () => {
    vi.mocked(desktop.listAllSteelModels).mockResolvedValue(page([model()]));
    renderPanel();
    expect(await screen.findByText("XGBoost")).toBeInTheDocument();
    expect(screen.getByText(/Q690 heats\.csv/)).toBeInTheDocument();
    expect(screen.getByText("0.9700")).toBeInTheDocument();
  });

  it("modelCardTitle and modelCardMetrics read manifest metrics", () => {
    const entry = model();
    expect(modelCardTitle(entry)).toBe("XGBoost");
    expect(modelCardMetrics(entry)?.r2).toBe(0.97);
  });

  it("activates a model version", async () => {
    vi.mocked(desktop.listAllSteelModels).mockResolvedValue(page([model()]));
    vi.mocked(desktop.setActiveSteelModel).mockResolvedValue({
      ...model({ is_active: true }),
      artifact_json: null,
      model_base64: null,
    });
    renderPanel();
    const button = await screen.findByRole("button", { name: "设为活动" });
    fireEvent.click(button);
    await waitFor(() => expect(desktop.setActiveSteelModel).toHaveBeenCalledWith("model-1"));
  });

  it("blocks deleting the active version", async () => {
    vi.mocked(desktop.listAllSteelModels).mockResolvedValue(page([model({ is_active: true })]));
    renderPanel();
    const deleteButton = await screen.findByRole("button", { name: "删除" });
    expect(deleteButton).toBeDisabled();
  });

  it("loads scientific cards for the selected model", async () => {
    const registered = model();
    vi.mocked(desktop.listAllSteelModels).mockResolvedValue(page([registered]));
    vi.mocked(desktop.getSteelModelScientificCards).mockResolvedValue({
      status: "recorded",
      persisted: true,
      cards: {
        cards_sha256: "b".repeat(64),
        model_id: registered.id,
        model_sha256: registered.model_sha256,
        cards: [],
      },
    } as never);
    renderPanel();
    fireEvent.click(await screen.findByRole("button", { name: "科研卡" }));
    await waitFor(() => expect(desktop.getSteelModelScientificCards).toHaveBeenCalledWith(registered.id));
  });
});
