import { useEffect, useState } from "react";
import { History } from "lucide-react";
import { desktop, type ComputeTaskRecord } from "../../bridge/desktop";
import { DataTable, type DataTableColumn } from "../../components/ui/data-table";
import { errorMessage } from "../../utils/errorMessage";

/**
 * 第 80 章：工艺优化任务的历史记录（来自 `optimization_tasks` 表）。
 *
 * 表里保存的是语义信息（数据集 / 方向 / 算法 / 搜索次数 / 状态），
 * 执行细节仍由 `background_tasks` 负责。
 */

const STATE_LABELS: Record<ComputeTaskRecord["state"], string> = {
  queued: "排队中",
  running: "运行中",
  succeeded: "已完成",
  failed: "失败",
  cancelled: "已取消",
};

const columns: DataTableColumn<ComputeTaskRecord>[] = [
  {
    id: "created",
    header: "创建时间",
    value: (task) => new Date(task.created_at).toLocaleString("zh-CN"),
  },
  { id: "dataset", header: "数据集", value: (task) => task.dataset_id ?? "—" },
  { id: "direction", header: "方向", value: (task) => (task.direction === "minimize" ? "最小化" : task.direction === "maximize" ? "最大化" : "—") },
  { id: "method", header: "算法", value: (task) => task.method ?? "自动" },
  { id: "trials", header: "搜索次数", value: (task) => task.trials_requested ?? 0, align: "right" },
  { id: "state", header: "状态", value: (task) => STATE_LABELS[task.state] ?? task.state },
  { id: "error", header: "错误", value: (task) => task.error_message ?? "—" },
];

export function OptimizationHistoryPanel() {
  const [tasks, setTasks] = useState<ComputeTaskRecord[]>([]);
  const [error, setError] = useState("");

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const items = await desktop.listComputeTasks("optimization");
        if (!cancelled) setTasks(items);
      } catch (reason) {
        if (!cancelled) setError(errorMessage(reason, "无法读取优化任务记录"));
      }
    })();
    return () => { cancelled = true; };
  }, []);

  return (
    <section className="suna-optimization-history" aria-label="历史优化任务">
      <h2><History size={16} />历史优化任务</h2>
      {error && <p className="suna-optimization-muted">{error}</p>}
      <DataTable
        columns={columns}
        rows={tasks}
        rowKey={(task) => task.id}
        pageSizeOptions={[10, 20, 50]}
        exportName="optimization-tasks"
        emptyText="还没有优化任务记录。运行一次优化后这里会留下可追溯的记录。"
      />
    </section>
  );
}
