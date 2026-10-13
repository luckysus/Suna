import { beforeEach, expect, it, vi } from "vitest";
import { desktop } from "../../bridge/desktop";
import { saveReportBlob } from "./reportExport";
vi.mock("../../bridge/desktop",()=>({desktop:{saveExportFile:vi.fn()}}));
beforeEach(()=>vi.clearAllMocks());
it("sends actual report bytes to native export and preserves cancellation",async()=>{
  vi.mocked(desktop.saveExportFile).mockResolvedValue({path:"C:\\研究\\报告.docx",bytes:3});
  expect(await saveReportBlob(new Blob(["abc"]),"研究/报告","docx","Word")).toBe(true);
  expect(desktop.saveExportFile).toHaveBeenCalledWith("研究_报告.docx",[97,98,99]);
  vi.mocked(desktop.saveExportFile).mockResolvedValue(null);
  expect(await saveReportBlob(new Blob(["abc"]),"报告","docx","Word")).toBe(false);
});
it("propagates native write errors instead of claiming success",async()=>{
  vi.mocked(desktop.saveExportFile).mockRejectedValue(new Error("permission denied"));
  await expect(saveReportBlob(new Blob(["abc"]),"报告","docx","Word")).rejects.toThrow("permission denied");
});
