import {act, fireEvent, render, screen, waitFor} from "@testing-library/react";
import {beforeEach, expect, it, vi} from "vitest";
import ResearchPackagePanel from "./ResearchPackagePanel";
import type {PackageInspection, PackagePreview, PackageService} from "../../../bridge/researchPackageTypes";
const preview: PackagePreview = {
  schemaVersion:"research-package-manifest.v1",scope:"metadata_only",sourceProjectId:"project-a",receiptSha256:"b".repeat(64),
  metadataRecordCount:1101,edgeCount:4,missingDependencies:[{resource_id:"model-a",kind:"model",status:"missing"}],
  assetsIncluded:false,provenanceMode:"references_only",rightsStatus:"not_assessed",writesToLiveWorkspace:false,assetsRestored:false,
  packageSha256:"a".repeat(64),bytes:350000,
};
const inspection: PackageInspection = {...preview,dryRun:{schemaVersion:"research-package-dry-run.v1",receiptSha256:preview.receiptSha256,
  sourceProjectId:preview.sourceProjectId,metadataRecordCount:1101,edgeCount:4,missingDependencies:preview.missingDependencies,
  status:"metadata_only_missing_assets",writesToLiveWorkspace:false,assetsRestored:false,idempotenceVerified:true,idMapping:{"project-a":"temporary-project"}}};
let service:PackageService;
beforeEach(() => {
  service={preview:vi.fn().mockResolvedValue(preview),export:vi.fn().mockResolvedValue({...preview,writeVerified:true}),inspectSelectedFile:vi.fn().mockResolvedValue(inspection)};
});
async function prepare() {
  fireEvent.click(screen.getByRole("button",{name:"预览导出内容"}));
  await screen.findByLabelText("科研包导出预览");
}
async function confirm() {
  await prepare(); fireEvent.click(screen.getByRole("checkbox",{name:"确认项目名称与成果标题可分享"}));
}
it("binds a confirmed export to the preview digest and waits for verified disk receipt",async()=>{
  render(<ResearchPackagePanel projectId="project-a" service={service}/>);await prepare();
  expect(screen.getByRole("button",{name:"选择位置并导出科研包"})).toBeDisabled();expect(service.export).not.toHaveBeenCalled();
  expect(screen.getByText(/外部资产：1 项/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox",{name:"确认项目名称与成果标题可分享"}));fireEvent.click(screen.getByRole("button",{name:"选择位置并导出科研包"}));
  expect(await screen.findByText("科研项目元数据包已保存并通过回读校验。")).toBeInTheDocument();
  expect(service.export).toHaveBeenCalledWith("project-a",preview.packageSha256);
});
it("reports native save cancellation without claiming export success",async()=>{
  vi.mocked(service.export).mockResolvedValue(null);render(<ResearchPackagePanel projectId="project-a" service={service}/>);await confirm();
  fireEvent.click(screen.getByRole("button",{name:"选择位置并导出科研包"}));await screen.findByText("导出已取消。");
  expect(screen.queryByText(/已保存并通过/)).not.toBeInTheDocument();expect(screen.getByRole("checkbox")).not.toBeChecked();
});
it("invalidates a changed source and never displays private backend error text",async()=>{
  vi.mocked(service.export).mockRejectedValue(new Error("package_preview_changed C:/synthetic-private/credential-sentinel"));
  render(<ResearchPackagePanel projectId="project-a" service={service}/>);await confirm();fireEvent.click(screen.getByRole("button",{name:"选择位置并导出科研包"}));
  expect(await screen.findByRole("alert")).toHaveTextContent("项目内容已变化");expect(screen.queryByLabelText("科研包导出预览")).not.toBeInTheDocument();
  expect(document.body.textContent).not.toContain("credential-sentinel");
});
it("rejects mismatched preview and unverified export receipts",async()=>{
  vi.mocked(service.preview).mockResolvedValue({...preview,sourceProjectId:"project-b"});
  render(<ResearchPackagePanel projectId="project-a" service={service}/>);fireEvent.click(screen.getByRole("button",{name:"预览导出内容"}));await screen.findByRole("alert");
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();expect(service.export).not.toHaveBeenCalled();
  vi.mocked(service.preview).mockResolvedValue(preview);vi.mocked(service.export).mockResolvedValue({...preview,packageSha256:"f".repeat(64),writeVerified:true});
  await confirm();fireEvent.click(screen.getByRole("button",{name:"选择位置并导出科研包"}));await screen.findByRole("alert");
  expect(screen.queryByText(/已保存并通过/)).not.toBeInTheDocument();
});
it("labels only an isolated metadata check as complete and keeps missing assets explicit",async()=>{
  render(<ResearchPackagePanel projectId="project-a" service={service}/>);fireEvent.click(screen.getByRole("button",{name:"选择科研包并临时检查"}));
  await screen.findByLabelText("科研包临时检查结果");expect(screen.getByText(/外部资产缺失 1 项/)).toBeInTheDocument();expect(screen.getByText(/完整资产恢复仍待验证/)).toBeInTheDocument();
  expect(service.export).not.toHaveBeenCalled();expect(service.preview).not.toHaveBeenCalled();
});
it("does not create an inspection report when the native file picker is cancelled",async()=>{
  vi.mocked(service.inspectSelectedFile).mockResolvedValue(null);render(<ResearchPackagePanel projectId="project-a" service={service}/>);
  fireEvent.click(screen.getByRole("button",{name:"选择科研包并临时检查"}));await screen.findByText("文件选择已取消。");
  expect(screen.queryByLabelText("科研包临时检查结果")).not.toBeInTheDocument();
});
it("does not apply an old preview after selecting a different project",async()=>{
  let resolve!:(p:PackagePreview)=>void;vi.mocked(service.preview).mockImplementation(()=>new Promise(r=>{resolve=r;}));
  const view=render(<ResearchPackagePanel projectId="project-a" service={service}/>);fireEvent.click(screen.getByRole("button",{name:"预览导出内容"}));
  await waitFor(()=>expect(service.preview).toHaveBeenCalled());view.rerender(<ResearchPackagePanel projectId="project-b" service={service}/>);
  await act(async()=>resolve(preview));expect(screen.queryByLabelText("科研包导出预览")).not.toBeInTheDocument();expect(service.export).not.toHaveBeenCalled();
});
