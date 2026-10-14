import {useEffect, useRef, useState} from "react";
import type {PackageInspection, PackagePreview, PackageService, PackageSummary} from "../../../bridge/researchPackageTypes";

function metadataOnly(p: PackageSummary): boolean {
  return p.schemaVersion === "research-package-manifest.v1" && p.scope === "metadata_only"
    && p.assetsIncluded === false && p.assetsRestored === false && p.writesToLiveWorkspace === false
    && p.provenanceMode === "references_only" && p.rightsStatus === "not_assessed";
}
function errorMessage(error: unknown): string {
  const message = String(error);
  if (message.includes("package_preview_changed")) return "项目内容已变化，请重新预览后导出。";
  if (message.includes("package_hash_mismatch")) return "包内内容与记录的摘要不一致，检查未通过。";
  if (message.includes("package_size_budget") || message.includes("package_expansion_budget")) return "科研包超过安全读取预算，检查未通过。";
  if (message.includes("unsupported_package_schema")) return "此文件不是受支持的科研项目元数据包。";
  if (message.includes("package_dependency_digest_conflict")) return "同一资源的历史摘要不一致，请核对来源后重新导出。";
  if (message.includes("package_export_write_failed")) return "文件未能保存并通过回读校验，请检查所选位置后重试。";
  return "操作未完成，请检查项目或科研包后重试。";
}
export default function ResearchPackagePanel({projectId, service}: {projectId: string; service: PackageService}) {
  const [preview, setPreview] = useState<PackagePreview | null>(null);
  const [inspection, setInspection] = useState<PackageInspection | null>(null);
  const [confirmed, setConfirmed] = useState(false), [busy, setBusy] = useState(false);
  const [error, setError] = useState(""), [notice, setNotice] = useState("");
  const request = useRef(0);
  useEffect(() => {
    request.current++; setPreview(null); setInspection(null); setConfirmed(false); setBusy(false); setError(""); setNotice("");
    return () => {request.current++;};
  }, [projectId]);
  async function perform(action: () => Promise<void>) {
    if (busy) return;
    const version = ++request.current;
    setBusy(true); setError(""); setNotice("");
    try {await action();}
    catch (e) {if (version === request.current) {setError(errorMessage(e)); setConfirmed(false); if (String(e).includes("package_preview_changed")) setPreview(null);}}
    finally {if (version === request.current) setBusy(false);}
  }
  async function prepare() {
    setPreview(null); setConfirmed(false);
    const version = request.current;
    const result = await service.preview(projectId);
    if (version !== request.current) return;
    if (!metadataOnly(result) || result.sourceProjectId !== projectId || !/^[a-f0-9]{64}$/.test(result.packageSha256)) throw new Error("package_preview_scope_invalid");
    setPreview(result);
  }
  async function save() {
    if (!preview || !confirmed) return;
    const version = request.current;
    const result = await service.export(projectId, preview.packageSha256);
    if (version !== request.current) return;
    if (result && (!metadataOnly(result) || result.sourceProjectId !== projectId || result.packageSha256 !== preview.packageSha256 || result.writeVerified !== true)) throw new Error("package_export_receipt_invalid");
    setConfirmed(false);
    setNotice(result ? "科研项目元数据包已保存并通过回读校验。" : "导出已取消。");
  }
  async function inspect() {
    setInspection(null);
    const version = request.current;
    const result = await service.inspectSelectedFile();
    if (version !== request.current) return;
    if (!result) {setNotice("文件选择已取消。"); return;}
    if (!metadataOnly(result) || result.dryRun.status !== "metadata_only_missing_assets" || result.dryRun.assetsRestored !== false
      || result.dryRun.writesToLiveWorkspace !== false || result.dryRun.idempotenceVerified !== true
      || result.dryRun.receiptSha256 !== result.receiptSha256 || result.dryRun.metadataRecordCount !== result.metadataRecordCount) throw new Error("package_inspection_scope_invalid");
    setInspection(result);
  }
  return <section aria-label="科研项目元数据包"><h3>科研项目元数据包</h3>
    <p>分享项目结构、成果摘要与来源关系。原始数据、模型文件和凭据需要另行提供。</p>
    {error && <p role="alert">{error}</p>}{notice && <p role="status">{notice}</p>}{busy && <p role="status">正在处理科研包…</p>}
    <button type="button" disabled={busy} onClick={() => void perform(prepare)}>预览导出内容</button>
    {preview && <div aria-label="科研包导出预览"><p>{preview.metadataRecordCount} 条元数据 · {preview.edgeCount} 条来源关系 · {preview.bytes} 字节</p>
      <p>待另行提供的外部资产：{preview.missingDependencies.length} 项。资源授权状态未评估。</p>
      <label><input type="checkbox" checked={confirmed} disabled={busy} onChange={e => setConfirmed(e.target.checked)}/>确认项目名称与成果标题可分享</label>
      <button type="button" disabled={busy || !confirmed} onClick={() => void perform(save)}>选择位置并导出科研包</button>
      <details><summary>包摘要</summary><code>{preview.packageSha256}</code></details></div>}
    <button type="button" disabled={busy} onClick={() => void perform(inspect)}>选择科研包并临时检查</button>
    {inspection && <div aria-label="科研包临时检查结果"><p>元数据结构与包内摘要校验通过，临时重复导入校验通过。</p>
      <p>{inspection.metadataRecordCount} 条元数据 · {inspection.edgeCount} 条来源关系 · 外部资产缺失 {inspection.missingDependencies.length} 项</p>
      <p>检查范围为临时副本；完整资产恢复仍待验证。</p>
      <details><summary>来源项目与包摘要</summary><p>{inspection.sourceProjectId}</p><code>{inspection.packageSha256}</code></details></div>}
  </section>;
}
