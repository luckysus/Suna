import {act,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {beforeEach,expect,it,vi} from "vitest";
import WorkflowPanel from "./WorkflowPanel";
import type {Dataset,Preview,Service,Template,WorkflowRun} from "./types";
const template:Template={schemaVersion:"research-workflow-template.v1",id:"11111111-1111-4111-8111-111111111111",revision:1,name:"合成双模型",
  models:[{name:"HV",algorithm:"linear_regression",targetColumn:2,featureColumns:[0,1],direction:"maximize",unit:"HV",splitPolicy:{kind:"random",validationFraction:0.2}},
    {name:"J",algorithm:"random_forest",targetColumn:3,featureColumns:[1,0],direction:"maximize",unit:"J",splitPolicy:{kind:"time",timeColumn:4,validationFraction:0.2}}],
  variables:[{name:"c",unit:"wt%",min:0.1,max:0.6},{name:"temperature",unit:"degC",min:700,max:750}],constraints:[],trials:20,seed:42,timeoutSeconds:300};
const dataset:Dataset={id:"synthetic-dataset",sourceName:"合成材料.csv",sourceSha256:"a".repeat(64),mappingState:"ready",columns:[
  {ordinal:0,originalName:"C",canonicalField:"c",unit:"wt%",inferredType:"number",min:0.1,max:0.6},
  {ordinal:1,originalName:"temp",canonicalField:"temperature",unit:"degC",inferredType:"number",min:700,max:750},
  ...["HV","J"].map((n,i)=>({ordinal:i+2,originalName:n,canonicalField:n.toLowerCase(),unit:n,inferredType:"number" as const,min:50,max:150})),
  {ordinal:4,originalName:"time",canonicalField:"time",unit:"s",inferredType:"number",min:1,max:6}]};
const preview:Preview={schemaVersion:"research-workflow-dry-run.v1",projectId:"project-a",datasetId:dataset.id,datasetSha256:dataset.sourceSha256,templateId:template.id,templateRevision:1,
  dryRunSha256:"b".repeat(64),networkRequired:false,validationStatus:"repository_validated_read_only",timeoutSeconds:300,materialRules:null,
  validatedModels:template.models.map(m=>({name:m.name,algorithm:m.algorithm,featureNames:["c","temperature"],targetSchema:{name:m.name,unit:m.unit||undefined},validRowCount:6,filteredRowCount:0,trainingPayloadSha256:"c".repeat(64)}))};
const run:WorkflowRun={projectId:"project-a",task:{id:"root-a",kind:"research_modeling_workflow",state:"queued",progress:0,attempt:0,error_code:null,cancel_requested:false,can_cancel:true,can_retry:false},nodes:[],
  templateId:template.id,templateRevision:1,definitionArtifactId:"definition-a",definitionContentSha256:"d".repeat(64),templateContentSha256:"e".repeat(64),approvedDeadline:"2026-10-14T00:00:00Z",result:null};
let service:Service,progress:(event:{id:string})=>void;
beforeEach(()=>{
  service={listDatasets:vi.fn().mockResolvedValue([dataset]),listTemplates:vi.fn().mockResolvedValue([{id:template.id,revision:1,name:template.name,contentSha256:"e".repeat(64)}]),
    getTemplate:vi.fn().mockResolvedValue({template,contentSha256:"e".repeat(64),validationStatus:"saved_schema_only",repositoryDryRunRequired:true}),
    saveTemplate:vi.fn().mockImplementation(async(t,expected)=>({template:{...t,revision:expected+1},contentSha256:"f".repeat(64),validationStatus:"saved_schema_only",repositoryDryRunRequired:true})),
    dryRun:vi.fn().mockResolvedValue(preview),start:vi.fn().mockResolvedValue(run),listRuns:vi.fn().mockResolvedValue([]),getRun:vi.fn().mockResolvedValue(run),
    cancelBackgroundTask:vi.fn().mockResolvedValue({}),retryBackgroundTask:vi.fn().mockResolvedValue({}),listenSchedulerProgress:vi.fn().mockImplementation(async(fn)=>{progress=fn;return vi.fn();})};
});
async function configured(){
  render(<WorkflowPanel projectId="project-a" service={service}/>);
  await screen.findByRole("option",{name:"合成材料.csv"});
  fireEvent.change(screen.getByLabelText("已保存模板"),{target:{value:`${template.id}@1`}});
  await waitFor(()=>expect(screen.getByLabelText("模板名称")).toHaveValue(template.name));
  fireEvent.change(screen.getByLabelText("工作流数据集"),{target:{value:dataset.id}});
}
async function inspected(){await configured();fireEvent.click(screen.getByRole("button",{name:"只读预览"}));await screen.findByLabelText("已校验运行预览");}
it("runs only after read-only preflight and explicit confirmation with frozen hashes",async()=>{
  await inspected();expect(service.start).not.toHaveBeenCalled();expect(screen.getByRole("button",{name:"确认并开始工作流"})).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox",{name:"确认创建训练任务、模型版本、优化结果和待确认计划"}));fireEvent.click(screen.getByRole("button",{name:"确认并开始工作流"}));
  await waitFor(()=>expect(service.start).toHaveBeenCalledWith(expect.objectContaining({projectId:"project-a",datasetId:dataset.id,datasetSha256:dataset.sourceSha256,templateId:template.id,templateRevision:1,dryRunSha256:preview.dryRunSha256,confirmed:true,nonce:expect.any(String)})));
  expect(await screen.findByText(/原任务 root-a/)).toBeInTheDocument();
  expect(service.saveTemplate).not.toHaveBeenCalled();
});
it("invalidates preview and saves a new immutable revision after changing one model direction",async()=>{
  await inspected();fireEvent.change(screen.getByLabelText("优化方向 2"),{target:{value:"minimize"}});
  expect(screen.queryByLabelText("已校验运行预览")).not.toBeInTheDocument();expect(screen.getByRole("button",{name:"只读预览"})).toBeDisabled();
  fireEvent.click(screen.getByRole("button",{name:"保存独立模板版本"}));
  await waitFor(()=>expect(service.saveTemplate).toHaveBeenCalledWith(expect.objectContaining({models:[expect.objectContaining({direction:"maximize"}),expect.objectContaining({direction:"minimize"})]}),1));
  await waitFor(()=>expect(screen.getByRole("button",{name:"只读预览"})).not.toBeDisabled());
});
it("does not expose private backend errors or present a failed preview as runnable",async()=>{
  vi.mocked(service.dryRun).mockRejectedValue(new Error("C:/synthetic-private/secret.csv credential-sentinel"));await configured();
  fireEvent.click(screen.getByRole("button",{name:"只读预览"}));expect(await screen.findByRole("alert")).toHaveTextContent("操作未完成");
  expect(screen.queryByRole("button",{name:"确认并开始工作流"})).not.toBeInTheDocument();expect(document.body.textContent).not.toContain("credential-sentinel");expect(service.start).not.toHaveBeenCalled();
});
it("rejects a preview that belongs to another project",async()=>{
  vi.mocked(service.dryRun).mockResolvedValue({...preview,projectId:"project-b"});await configured();fireEvent.click(screen.getByRole("button",{name:"只读预览"}));
  await screen.findByRole("alert");expect(screen.queryByLabelText("已校验运行预览")).not.toBeInTheDocument();expect(service.start).not.toHaveBeenCalled();
});
it("refreshes restored runs from Scheduler events and cancels through the existing task API",async()=>{
  vi.mocked(service.listRuns).mockResolvedValue([run]);render(<WorkflowPanel projectId="project-a" service={service}/>);
  fireEvent.click(await screen.findByRole("button",{name:"取消工作流"}));await waitFor(()=>expect(service.cancelBackgroundTask).toHaveBeenCalledWith("root-a"));
  await waitFor(()=>expect(service.getRun).toHaveBeenCalledWith("project-a","root-a"));
  const done={...run,task:{...run.task,state:"completed" as const,progress:100,can_cancel:false}};vi.mocked(service.listRuns).mockResolvedValue([done]);
  act(()=>progress({id:"root-a"}));expect(await screen.findByText(/已完成 · 100%/)).toBeInTheDocument();expect(screen.queryByRole("button",{name:"取消工作流"})).not.toBeInTheDocument();
});
it("does not apply an old project preview after switching projects",async()=>{
  let resolve!:(v:Preview)=>void;vi.mocked(service.dryRun).mockImplementation(()=>new Promise(r=>{resolve=r;}));
  const view=render(<WorkflowPanel projectId="project-a" service={service}/>);await screen.findByRole("option",{name:"合成材料.csv"});
  fireEvent.change(screen.getByLabelText("已保存模板"),{target:{value:`${template.id}@1`}});await waitFor(()=>expect(screen.getByLabelText("模板名称")).toHaveValue(template.name));
  fireEvent.change(screen.getByLabelText("工作流数据集"),{target:{value:dataset.id}});fireEvent.click(screen.getByRole("button",{name:"只读预览"}));await waitFor(()=>expect(service.dryRun).toHaveBeenCalled());
  view.rerender(<WorkflowPanel projectId="project-b" service={service}/>);await act(async()=>resolve(preview));
  expect(screen.queryByLabelText("已校验运行预览")).not.toBeInTheDocument();expect(service.start).not.toHaveBeenCalled();
});
