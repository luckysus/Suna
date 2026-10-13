import {render,screen,fireEvent} from "@testing-library/react";
import {expect,it,vi} from "vitest";
import {desktop} from "../../bridge/desktop";
import ComputeWorkerDiagnostics from "./ComputeWorkerDiagnostics";
vi.mock("../../bridge/desktop",()=>({desktop:{diagnoseComputeWorker:vi.fn()}}));
it("loads actual dependency status only on request and exposes partial failure",async()=>{
  vi.mocked(desktop.diagnoseComputeWorker).mockResolvedValue({state:"degraded",networkUsed:false,workerVersion:"0.6",protocolVersion:"1.0",dependencies:[{name:"pillow",status:"unavailable",version:null,errorCode:"dependency_missing"}]});
  render(<ComputeWorkerDiagnostics/>);expect(desktop.diagnoseComputeWorker).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"检查计算 Worker"}));
  expect(await screen.findByRole("status")).toHaveTextContent("部分计算依赖不可用");
  expect(screen.getByText("dependency_missing")).toBeVisible();
});
