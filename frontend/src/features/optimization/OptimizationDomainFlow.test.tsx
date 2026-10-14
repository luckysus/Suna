import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { desktop, type ComputeOptimizationResult, type ComputeTrainingResult, type SteelDatasetRecord } from "../../bridge/desktop";
import { useResearchStore } from "../../stores/researchStore";
import ProcessOptimizationPage from "./ProcessOptimizationPage";

vi.mock("../../bridge/desktop",()=>({desktop:{listSteelDatasets:vi.fn(),listBackgroundTasks:vi.fn(),getComputeTrainingResult:vi.fn(),listComputeTasks:vi.fn(),
  optimizeSteelProcess:vi.fn(),getComputeOptimizationResult:vi.fn(),getBackgroundTask:vi.fn(),listenSchedulerProgress:vi.fn(),cancelBackgroundTask:vi.fn(),retryBackgroundTask:vi.fn()}}));
vi.mock("../../hooks/useNativeFileDrop",()=>({useNativeFileDrop:vi.fn()}));

const dataset={id:"dataset",sourceName:"合成.csv",mappingState:"ready",columns:[{originalName:"C",canonicalField:"c",unit:"wt%",inferredType:"number",duplicate:false,min:.1,max:.3}]} as SteelDatasetRecord;
const trained:ComputeTrainingResult={task_id:"model-task",state:"completed",artifact:{model_id:"a".repeat(24),model_type:"linear_regression",feature_names:["c"],metrics:{},applicability_range:[{min:.1,max:.3}]}};
function result():ComputeOptimizationResult {
  return {task_id:"optimization",state:"completed",method:"nsga2",direction:"maximize",directions:["maximize"],objectives:["HV"],feature_names:["c"],model_id:"joint",model_type:"joint",trials_completed:1,deterministic_seed:42,
    recommendations:[],pareto_front:[],domain_filter:{policy:"within_domain_only",scope:"returned_pareto_front",before:1,after:0,excluded:1,status:"insufficient_domain_candidates",joint_probability:null,physical_constraints_changed:false}};
}
beforeEach(()=>{
  vi.clearAllMocks();localStorage.clear();window.history.replaceState({},"","/");useResearchStore.setState({currentProject:null});
  vi.mocked(desktop.listSteelDatasets).mockResolvedValue([dataset]);
  vi.mocked(desktop.listBackgroundTasks).mockResolvedValue([{id:"model-task",kind:"compute_train_linear_regression",state:"completed",dataset_id:"dataset"}] as never);
  vi.mocked(desktop.getComputeTrainingResult).mockResolvedValue(trained);vi.mocked(desktop.listComputeTasks).mockResolvedValue([]);
  vi.mocked(desktop.optimizeSteelProcess).mockResolvedValue({id:"optimization"} as never);
  vi.mocked(desktop.getBackgroundTask).mockResolvedValue({id:"optimization",state:"completed",progress:100} as never);
  vi.mocked(desktop.listenSchedulerProgress).mockResolvedValue(()=>{});vi.mocked(desktop.getComputeOptimizationResult).mockResolvedValue(result());
});

it("forwards the explicit domain policy through the real bridge and handles a completed empty front",async()=>{
  render(<ProcessOptimizationPage/>);await screen.findByLabelText("c 下限");
  expect(screen.getByLabelText("候选筛选策略")).toHaveValue("annotate_all");expect(desktop.optimizeSteelProcess).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("候选筛选策略"),{target:{value:"within_domain_only"}});
  fireEvent.click(screen.getByRole("button",{name:"开始优化"}));
  await screen.findByText(/当前结果没有可加入实验计划的候选/);
  expect(desktop.optimizeSteelProcess).toHaveBeenCalledWith(expect.objectContaining({datasetId:"dataset",domainPolicy:"within_domain_only",constraints:[],seed:42}));
  expect(screen.queryByRole("button",{name:"加入实验计划"})).not.toBeInTheDocument();
});

it("keeps the default annotation policy and shows per-model evidence returned by the durable task",async()=>{
  const outcome=result();outcome.domain_filter={...outcome.domain_filter!,policy:"annotate_all",after:1,excluded:0,status:"completed"};
  const candidate={values:{c:.3},objectives:[110],prediction:110,feasible:true,constraint_residuals:{},model_applicability:[{objective_index:0,candidate_index:0,model_id:"a".repeat(24),model_sha256:"d".repeat(64),dataset_sha256:"c".repeat(64),support_sha256:"b".repeat(64),schema_version:"applicability-support.v1" as const,method:"marginal_training_ranges" as const,probability:null,physical_feasibility:"not_assessed" as const,joint_support:"not_assessed" as const,interval_status:"unknown" as const,risk:{row_index:0,state:"outside_domain" as const,reasons:[{state:"outside_domain" as const,code:"outside_training_range",feature:"c"}]}}]};
  outcome.recommendations=[candidate];outcome.pareto_front=[candidate];vi.mocked(desktop.getComputeOptimizationResult).mockResolvedValue(outcome);
  render(<ProcessOptimizationPage/>);await screen.findByLabelText("c 下限");fireEvent.click(screen.getByRole("button",{name:"开始优化"}));
  await screen.findByText("HV：超出训练范围");
  expect(desktop.optimizeSteelProcess).toHaveBeenCalledWith(expect.objectContaining({domainPolicy:"annotate_all"}));
  await waitFor(()=>expect(screen.getByRole("button",{name:"加入实验计划"})).toBeInTheDocument());
});
