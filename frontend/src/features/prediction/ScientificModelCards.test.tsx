import { act, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ScientificModelCards from "./ScientificModelCards";
import type { ScientificCardsResponse } from "../../bridge/desktop";

const first="11111111-1111-4111-8111-111111111111";
const second="22222222-2222-4222-8222-222222222222";
const sha="a".repeat(64);
const metric={sample_count:null,rmse:null,mae:null,r2:null};
function cards(id=first,version=1):ScientificCardsResponse {
  return {status:"recorded",persisted:true,cards:{cards_sha256:"b".repeat(64),model_card:{
    schema_version:"model-card.v1",registry_id:id,version,model_sha256:sha,algorithm:"svr",parameters:{c:10},
    features:[{name:"c",unit:"wt%"},{name:"temperature",unit:null}],target:{name:"hv",unit:"HV"},
    split_kind:"group",split_counts:{train:10,validation:4,test:null,calibration:null},
    metrics:{train:metric,validation:metric,test:metric},interval:{status:"unknown",nominal_coverage:null,observed_test_coverage:null,exchangeability_verified:false},
    dataset_sha256:null,review_status:"unreviewed"},data_card:{schema_version:"data-card.v1",dataset_id:null,dataset_sha256:null,
    filtered_row_count:null,valid_row_count:14,source_row_count:null,filter_policy:"unknown",grouping_field:"furnace",time_field:null,imputation:"train_mean",quality_status:"unknown"}}};
}

describe("ScientificModelCards independent interactions",()=>{
  it("loads the selected immutable version and keeps missing test/coverage metadata unknown",async()=>{
    const load=vi.fn().mockResolvedValue(cards());
    render(<ScientificModelCards modelId={first} modelSha={sha} loadCards={load}/>);
    expect(await screen.findByText("科研模型卡 · v1")).toBeInTheDocument();
    expect(load).toHaveBeenCalledWith(first);
    expect(screen.getByText("分组划分")).toBeInTheDocument();
    expect(screen.getByText("测试集实际覆盖率").nextElementSibling).toHaveTextContent("未知");
    expect(screen.getByText("temperature · 单位未知")).toBeInTheDocument();
    expect(screen.getByText(/该假设尚未验证/)).toBeInTheDocument();
    expect(screen.getByText("数据质量").nextElementSibling).toHaveTextContent("未知，尚未审核");
  });
  it("shows legacy derivation and refuses a card from a different registry ID",async()=>{
    const legacy=cards();legacy.status="legacy_derived";legacy.persisted=false;
    const load=vi.fn().mockResolvedValueOnce(legacy).mockResolvedValueOnce(cards(first));
    const rendered=render(<ScientificModelCards modelId={first} modelSha={sha} loadCards={load}/>);
    expect(await screen.findByText(/旧模型只读生成，未改写原记录/)).toBeInTheDocument();
    rendered.rerender(<ScientificModelCards modelId={second} modelSha={sha} loadCards={load}/>);
    expect(await screen.findByRole("alert")).toHaveTextContent("科研卡与当前模型版本不匹配");
    expect(screen.queryByText("科研模型卡 · v1")).not.toBeInTheDocument();
  });
  it("does not replace a newly selected model with an earlier slow response",async()=>{
    let resolveFirst!:(value:ScientificCardsResponse)=>void;
    const slow=new Promise<ScientificCardsResponse>(resolve=>{resolveFirst=resolve;});
    const load=vi.fn().mockReturnValueOnce(slow).mockResolvedValueOnce(cards(second,2));
    const rendered=render(<ScientificModelCards modelId={first} modelSha={sha} loadCards={load}/>);
    rendered.rerender(<ScientificModelCards modelId={second} modelSha={sha} loadCards={load}/>);
    expect(await screen.findByText("科研模型卡 · v2")).toBeInTheDocument();
    await act(async()=>{resolveFirst(cards(first,1));});
    await waitFor(()=>expect(screen.queryByText("科研模型卡 · v1")).not.toBeInTheDocument());
    expect(screen.getByText("科研模型卡 · v2")).toBeInTheDocument();
  });
  it("keeps backend errors and model-path details out of the UI",async()=>{
    const load=vi.fn().mockRejectedValue(new Error("C:/synthetic-private/model.bin"));
    render(<ScientificModelCards modelId={first} modelSha={sha} loadCards={load}/>);
    expect(await screen.findByRole("alert")).toHaveTextContent("无法读取经过校验的科研卡");
    expect(screen.queryByText(/synthetic-private/)).not.toBeInTheDocument();
  });
});
