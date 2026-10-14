import type { ModelDomainEvidence } from "../../bridge/desktop";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CandidateDomainEvidence, OptimizationDomainPolicy, OptimizationDomainSummary } from "./OptimizationDomainEvidence";

function evidence(index: number, state: "within_domain" | "outside_domain"): ModelDomainEvidence {
  return { objective_index:index, candidate_index:0, model_id:"a".repeat(24), model_sha256:"d".repeat(64), support_sha256:"b".repeat(64),dataset_sha256:"c".repeat(64),
    schema_version:"applicability-support.v1",method:"marginal_training_ranges",probability:null,physical_feasibility:"not_assessed",joint_support:"not_assessed",interval_status:"requires_exchangeability_assumption",
    risk:{row_index:0,state,reasons:state==="outside_domain"?[{state,code:"outside_training_range",feature:"C"}]:[]}};
}
describe("joint candidate scientific evidence",()=>{
  it("keeps annotation default and requires an explicit user selection for filtering",()=>{
    const onChange=vi.fn();render(<OptimizationDomainPolicy value="annotate_all" onChange={onChange}/>);
    expect(screen.getByLabelText("候选筛选策略")).toHaveValue("annotate_all");expect(onChange).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("候选筛选策略"),{target:{value:"within_domain_only"}});
    expect(onChange).toHaveBeenCalledWith("within_domain_only");expect(screen.getByText(/不能保证材料物理可行性/)).toBeInTheDocument();
  });
  it("renders each independent model and its own risk without a joint confidence",()=>{
    render(<CandidateDomainEvidence objectives={["HV","J"]} entries={[evidence(0,"within_domain"),evidence(1,"outside_domain")]}/>);
    expect(screen.getByText("HV：通过训练范围筛查")).toBeInTheDocument();expect(screen.getByText("J：超出训练范围")).toBeInTheDocument();
    expect(screen.getByText(/C：超过训练集数值范围/)).toBeInTheDocument();expect(screen.getAllByText(/外推覆盖率未验证/)).toHaveLength(2);
  });
  it("reports an empty explicit filter result with before/after evidence",()=>{
    render(<OptimizationDomainSummary filter={{policy:"within_domain_only",scope:"returned_pareto_front",before:8,after:0,excluded:8,status:"insufficient_domain_candidates",joint_probability:null,physical_constraints_changed:false}}/>);
    expect(screen.getByRole("status")).toHaveTextContent("前沿 8 个，保留 0 个，排除 8 个");expect(screen.getByText(/没有可加入实验计划的候选/)).toBeInTheDocument();
    expect(screen.getByText(/不构成已校准的联合置信度/)).toBeInTheDocument();
  });
  it("keeps old results unknown and formula-only results without model claims",()=>{
    const view=render(<CandidateDomainEvidence objectives={["HV"]}/>);expect(screen.getByText(/历史任务未记录/)).toBeInTheDocument();
    view.rerender(<CandidateDomainEvidence objectives={["cost"]} entries={[]}/>);expect(screen.getByText(/公式目标没有预测模型支持域证据/)).toBeInTheDocument();
  });
  it("shows the model-specific threshold units and full source hashes",()=>{
    const entry=evidence(1,"outside_domain");
    entry.risk.reasons[0].threshold={min:0.001,max:0.002,unit:"fraction",source:"training_split",support_sha256:entry.support_sha256!};
    render(<CandidateDomainEvidence objectives={["HV","J"]} entries={[entry]}/>);
    expect(screen.getByText(/0.001 – 0.002 fraction/)).toHaveTextContent("来源：训练子集");
    expect(screen.getByText(`训练支持域 SHA：${entry.support_sha256}`)).toBeInTheDocument();
    expect(screen.getByText(`训练数据 SHA：${entry.dataset_sha256}`)).toBeInTheDocument();
  });
});
