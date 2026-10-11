import { FileText, Trash2 } from "lucide-react";
import type { ReportRecord } from "../../bridge/desktop";
import { Button } from "../../components/ui/button";

/** 第 56 / 80 章：已保存的科研报告（落库在 `reports` 表）。 */
export function SavedReportsPanel({ reports, busy, onOpen, onDelete }: {
  reports: ReportRecord[];
  busy: boolean;
  onOpen: (report: ReportRecord) => void;
  onDelete: (report: ReportRecord) => void;
}) {
  if (reports.length === 0) {
    return <p className="suna-reports-saved-empty">还没有保存过报告。生成后点「保存报告」即可留档。</p>;
  }
  return (
    <ul className="suna-reports-saved-list">
      {reports.map((report) => (
        <li key={report.id}>
          <FileText size={14} aria-hidden="true" />
          <span>
            <strong>{report.title}</strong>
            <small>{report.kind} · {new Date(report.created_at).toLocaleString("zh-CN")}</small>
          </span>
          <Button variant="ghost" onClick={() => onOpen(report)} disabled={busy}>打开</Button>
          <Button size="icon" aria-label={`删除 ${report.title}`} title="删除" onClick={() => onDelete(report)} disabled={busy}>
            <Trash2 size={14} />
          </Button>
        </li>
      ))}
    </ul>
  );
}
