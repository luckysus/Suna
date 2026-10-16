import { useState } from "react";
import { desktop } from "../../bridge/desktop";
import type { LiteratureEvidenceRecord } from "./evidence";
import { Button } from "../../components/ui/button";
import { Input } from "../../components/ui/input";

export default function LiteratureEvidenceTable({ records, onChange, onExtract, onError, onNotice }: { records: LiteratureEvidenceRecord[]; onChange: (v: LiteratureEvidenceRecord[]) => void; onExtract: () => Promise<void>; onError: (s:string) => void; onNotice: (s:string) => void }) {
  const [preview, setPreview] = useState<{ record: LiteratureEvidenceRecord; content: string } | null>(null);
  const open = async (record: LiteratureEvidenceRecord) => {
    if (!record.documentId) { onError("没有本地原文，不能确认审核"); return; }
    try {
      if (!record.knowledgeBaseId) throw new Error("知识库身份缺失，不能审核");
      const documents = await desktop.listKnowledgeDocuments(record.knowledgeBaseId);
      if (documents.find((d) => d.id === record.documentId)?.active_version_id !== record.versionId) throw new Error("原文版本已变化，请重新检索");
      const versions = await desktop.listDocumentVersions(record.documentId);
      if (!versions.some((v) => v.id === record.versionId)) throw new Error("证据版本不存在，不能审核");
      // The preview API returns the active version; reject a changed source.
      const original = await desktop.getKnowledgeDocumentPreview(record.documentId);
      if (original.metadata?.version_id && original.metadata.version_id !== record.versionId) throw new Error("原文版本已变化，请重新检索");
      const content = original.content || original.blocks.map((b) => b.content).join("\n");
      if (!content.includes(record.quote)) throw new Error("当前原文未找到引用，请核对版本和来源");
      setPreview({ record, content });
    } catch (e) { onError(e instanceof Error ? e.message : String(e)); }
  };
  const exportRecords = async (importDataset = false) => {
    try {
      const reviewed = records.filter((r) => r.reviewed && r.field && r.value);
      if (!reviewed.length) throw new Error("请先查看原文并审核至少一条完整数据记录");
      const rows = reviewed.map((r) => ({ field:r.field, value: Number.isFinite(Number(r.value)) ? Number(r.value) : r.value, unit:r.unit, doi:r.doi ?? "", document_id:r.documentId, version_id:r.versionId, chunk_id:r.chunkId, source:r.source, source_location:JSON.stringify(r.location), evidence:r.quote, conclusion:r.conclusion }));
      const receipt = await desktop.saveExportFile("已审核文献数据.json", Array.from(new TextEncoder().encode(JSON.stringify(rows,null,2))));
      if (!receipt) return;
      if (importDataset) { await desktop.saveSteelDataset({ sourcePath: receipt.path }); onNotice("已审核记录已加入数据实验室，请完成字段映射后使用"); }
      else onNotice(`已审核证据已导出：${receipt.path}`);
    } catch (e) { onError(e instanceof Error ? e.message : String(e)); }
  };
  if (!records.length) return null;
  return <section className="suna-literature-evidence-section"><h2>文献证据表（AI 结论需原文审核）</h2><div className="flex flex-wrap gap-2 my-3"><Button onClick={() => void onExtract()}>提取成分 / 工艺 / 性能字段</Button><Button onClick={() => void exportRecords()}>导出已审核证据</Button><Button onClick={() => void exportRecords(true)}>加入数据实验室</Button></div>
    <div style={{overflowX:"auto"}}><table className="suna-literature-evidence-table"><thead><tr>{["编号","DOI / 原文","页码 / 章节","原文证据","提取字段","值","单位","AI 结论","审核"].map((s) => <th key={s}>{s}</th>)}</tr></thead><tbody>{records.map((r,i) => {
      const update = (patch: Partial<LiteratureEvidenceRecord>) => onChange(records.map((row,j) => j === i ? {...row,...patch,reviewed:false} : row));
      return <tr key={r.id}><td>{r.id}</td><td>{r.doi ?? "DOI 未记录"}<br/>{r.source}<br/>{r.versionId ?? "版本未记录"}<Button onClick={() => void open(r)}>核对原文</Button></td><td>{r.location ? JSON.stringify(r.location) : "位置未记录"}</td><td>{r.quote}</td><td><Input aria-label={`${r.id} 字段`} value={r.field} onChange={(e) => update({field:e.target.value})} /></td><td><Input aria-label={`${r.id} 值`} value={r.value} onChange={(e) => update({value:e.target.value})} /></td><td><Input aria-label={`${r.id} 单位`} value={r.unit} onChange={(e) => update({unit:e.target.value})} /></td><td>{r.conclusion || "没有关联证据的结论"}</td><td>{r.reviewed ? "已人工审核" : "待审核"}</td></tr>;
    })}</tbody></table></div>
    {preview && <div role="dialog" aria-label="证据原文审核"><h3>{preview.record.source}</h3><p>引用：{preview.record.quote}</p><pre style={{maxHeight:300,overflow:"auto",whiteSpace:"pre-wrap"}}>{preview.content}</pre><Button onClick={() => { onChange(records.map((r) => r.id === preview.record.id ? {...r,reviewed:true} : r)); setPreview(null); }}>确认原文与提取字段一致</Button><Button onClick={() => setPreview(null)}>取消</Button></div>}
  </section>;
}
