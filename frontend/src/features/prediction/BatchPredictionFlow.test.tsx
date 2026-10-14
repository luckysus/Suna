import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { desktop, type ComputeTrainingResult } from "../../bridge/desktop";
import BatchPredictionPanel from "./BatchPredictionPanel";

vi.mock("../../bridge/desktop", () => ({ desktop: {
  listBackgroundTasks: vi.fn(), getComputeTrainingResult: vi.fn(), getSetting: vi.fn(), setSetting: vi.fn(),
  predictSteelModel: vi.fn(), getComputePredictionResult: vi.fn(), listenSchedulerProgress: vi.fn(),
  getBackgroundTask: vi.fn(), saveExportFile: vi.fn(), cancelBackgroundTask: vi.fn(), retryBackgroundTask: vi.fn(),
} }));
const model: ComputeTrainingResult = { task_id: "model-one", state: "completed", artifact: {
  model_id: "model", model_type: "linear_regression", feature_names: ["c", "temperature"], metrics: {}, applicability_range: [],
} };
beforeEach(() => {
  vi.clearAllMocks(); localStorage.clear();
  vi.mocked(desktop.listBackgroundTasks).mockResolvedValue([]);
  vi.mocked(desktop.getSetting).mockResolvedValue(null);
  vi.mocked(desktop.setSetting).mockResolvedValue(undefined);
  vi.mocked(desktop.listenSchedulerProgress).mockResolvedValue(() => {});
  vi.mocked(desktop.predictSteelModel).mockResolvedValue({ id: "prediction-one" } as never);
  vi.mocked(desktop.getBackgroundTask).mockResolvedValue({ id: "prediction-one", state: "completed", progress: 100 } as never);
  vi.mocked(desktop.saveExportFile).mockResolvedValue({ path: "synthetic.csv" } as never);
});

it("submits declared order/units and displays and exports normalized model values", async () => {
  vi.mocked(desktop.getComputePredictionResult).mockResolvedValue({ task_id: "prediction-one", state: "completed", model_id: "model", model_type: "linear_regression", feature_names: ["c", "temperature"], input_values: [0.2, 810], input_rows: [[0.2, 810]], predictions: [105], applicability_range: [], applicability_warnings: [], confidence: null, constraints: [], applicability: { schema_version: "applicability-support.v1", method: "marginal_training_ranges", model_id: "model", support_sha256: "a".repeat(64), probability: null, physical_feasibility: "not_assessed", joint_support: "not_assessed", rows: [{ row_index: 0, state: "within_domain", reasons: [] }] } });
  render(<BatchPredictionPanel datasetId="dataset" model={model} onError={vi.fn()} onNotice={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("批量预测输入"), { target: { value: JSON.stringify({ schema: [{ name: "temperature", unit: "K" }, { name: "c", unit: "fraction" }], rows: [[1083.15, 0.002]] }) } });
  fireEvent.click(screen.getByRole("button", { name: "提交整批预测" }));
  await screen.findByText("通过训练范围筛查");
  expect(desktop.predictSteelModel).toHaveBeenCalledWith({ datasetId: "dataset", trainingTaskId: "model-one", featureRows: [[1083.15, 0.002]], inputSchema: [{ name: "temperature", unit: "K" }, { name: "c", unit: "fraction" }] });
  expect(screen.getByRole("cell", { name: "0.2" })).toBeInTheDocument();
  expect(screen.getByRole("cell", { name: "810" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "导出预测 CSV" }));
  await waitFor(() => expect(desktop.saveExportFile).toHaveBeenCalled());
  const csv = new TextDecoder().decode(new Uint8Array(vi.mocked(desktop.saveExportFile).mock.calls[0][1]));
  expect(csv).toContain("0.2,810,105"); expect(csv).toContain("within_domain"); expect(csv).not.toContain("1083.15");
});

it("shows legacy results as unknown instead of claiming range membership", async () => {
  vi.mocked(desktop.getComputePredictionResult).mockResolvedValue({ task_id: "prediction-one", state: "completed", model_id: "model", model_type: "linear_regression", feature_names: ["c", "temperature"], input_values: [0.2, 810], predictions: [105], applicability_range: [], applicability_warnings: [], confidence: null, constraints: [] });
  render(<BatchPredictionPanel datasetId="dataset" model={model} onError={vi.fn()} onNotice={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("批量预测输入"), { target: { value: "[[0.2,810]]" } });
  fireEvent.click(screen.getByRole("button", { name: "提交整批预测" }));
  await screen.findByText("适用域未知");
  expect(screen.queryByText("通过训练范围筛查")).not.toBeInTheDocument();
});
