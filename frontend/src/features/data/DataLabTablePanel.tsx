import { Database } from "lucide-react";
import type { DatasetSeries } from "../../bridge/desktop";
import { DataTable } from "../../components/ui/data-table";

/** 第 67 章：数据实验室的「数据表」页签——排序 / 筛选 / 分页 / 复制 / 导出。 */
export function DataLabTablePanel({ series, datasetName, busy }: {
  series: DatasetSeries | null;
  datasetName?: string;
  busy: boolean;
}) {
  if (!series) {
    return (
      <div className="suna-data-empty large">
        <Database size={30} />
        <strong>{busy ? "正在读取数据表…" : "尚未加载数据表"}</strong>
        <span>选择数据集后即可排序、筛选、分页、复制与导出。</span>
      </div>
    );
  }
  return (
    <DataTable
      columns={series.columns.map((column, index) => ({
        id: String(column.ordinal),
        header: column.unit ? `${column.name} (${column.unit})` : column.name,
        value: (row: { index: number; values: Array<number | null> }) => row.values[index],
        align: "right" as const,
      }))}
      rows={series.rows.map((values, index) => ({ index, values }))}
      rowKey={(row) => String(row.index)}
      title={`${datasetName ?? "数据表"}${series.sampled ? "（抽样预览）" : ""}`}
      exportName={(datasetName ?? "dataset").replace(/\.[^.]+$/, "")}
      emptyText="该数据集没有可展示的行"
    />
  );
}
