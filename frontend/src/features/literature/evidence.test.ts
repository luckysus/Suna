import { expect, it } from "vitest";
import { evidenceRecords, evidencePrompt, applyEvidenceExtraction, bindConclusions } from "./evidence";
const records = evidenceRecords([{ document_id:"doc", version_id:"v1", chunk_id:"chunk", source_name:"paper.pdf", text:"硬度为 300 HV。", source_location:{page:3} }], { doc:{doi:"10.123/example"} });
it("preserves source identity and rejects invented evidence IDs and quotations", () => {
  expect(evidencePrompt(records)).toContain("10.123/example");
  expect(records[0].versionId).toBe("v1");
  expect(records[0].location).toEqual({page:3});
  expect(() => bindConclusions(records,"结论 [E2]")).toThrow();
  expect(() => applyEvidenceExtraction(records, JSON.stringify([{evidenceId:"E1",quote:"硬度为 900 HV。",field:"hardness",value:"900",unit:"HV",conclusion:"高硬度"}]))).toThrow();
});
it("links conclusions explicitly and keeps extracted data unreviewed", () => {
  expect(bindConclusions(records,"硬度为300 HV [E1]")[0].conclusion).toContain("[E1]");
  const extracted=applyEvidenceExtraction(records,JSON.stringify([{evidenceId:"E1",quote:"硬度为 300 HV。",field:"hardness",value:"300",unit:"HV",conclusion:"高硬度"}]));
  expect(extracted[0]).toMatchObject({documentId:"doc",versionId:"v1",chunkId:"chunk",reviewed:false,value:"300"});
});
