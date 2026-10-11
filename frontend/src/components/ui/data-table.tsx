import { useEffect, useMemo, useState, type ReactNode } from "react";
import { ArrowDown, ArrowUp, ArrowUpDown, Check, Copy, Download, Search } from "lucide-react";
import { cn } from "../../lib/utils";
import { Input } from "./input";
import {
  clampPage,
  filterRows,
  pageCount,
  slicePage,
  sortRows,
  toCsv,
  toDelimited,
  toggleSort,
  type SortState,
} from "./dataTableModel";
import "./data-table.css";
import { Button } from "./button";

/**
 * 第 67 章：科研数据表格。
 *
 * 统一提供排序 / 筛选 / 分页 / 复制 / 导出，避免各页面各写一套。
 */

export type DataTableColumn<T> = {
  id: string;
  header: string;
  /** 取值函数：同时用于排序、筛选、复制与导出。 */
  value: (row: T) => string | number | null;
  /** 自定义单元格渲染；缺省渲染取值。 */
  render?: (row: T) => ReactNode;
  align?: "left" | "right";
  sortable?: boolean;
};

export function DataTable<T>({
  columns,
  rows,
  rowKey,
  title,
  searchable = true,
  pageSizeOptions = [20, 50, 100],
  exportName = "data",
  emptyText = "暂无数据",
  onRowClick,
}: {
  columns: DataTableColumn<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  title?: string;
  searchable?: boolean;
  pageSizeOptions?: number[];
  exportName?: string;
  emptyText?: string;
  onRowClick?: (row: T) => void;
}) {
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortState | null>(null);
  const [pageSize, setPageSize] = useState(pageSizeOptions[0] ?? 20);
  const [page, setPage] = useState(1);
  const [copied, setCopied] = useState(false);

  const view = useMemo(() => {
    const filtered = filterRows(rows, (row) => columns.map((column) => column.value(row) ?? "").join(" "), query);
    if (!sort) return filtered;
    const target = columns.find((column) => column.id === sort.columnId);
    if (!target) return filtered;
    return sortRows(filtered, (row) => target.value(row), sort.direction);
  }, [columns, query, rows, sort]);

  useEffect(() => { setPage(1); }, [query, pageSize, rows]);

  const totalPages = pageCount(view.length, pageSize);
  const current = clampPage(page, view.length, pageSize);
  const visible = slicePage(view, current, pageSize);
  const headers = columns.map((column) => column.header);
  const matrix = view.map((row) => columns.map((column) => column.value(row)));

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(toDelimited(headers, matrix));
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch {
      /* 剪贴板不可用时保持静默，导出仍可用 */
    }
  };

  const exportCsv = () => {
    const blob = new Blob([toCsv(headers, matrix)], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `${exportName}.csv`;
    anchor.click();
    URL.revokeObjectURL(url);
  };

  return (
    <section className="suna-data-table">
      <div className="suna-data-table-toolbar">
        {title && <strong>{title}</strong>}
        {searchable && (
          <label className="suna-data-table-search">
            <Search size={14} aria-hidden="true" />
            <Input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="筛选表格内容" aria-label="筛选表格内容" />
          </label>
        )}
        <span className="suna-data-table-count">{view.length} / {rows.length} 行</span>
        <div className="suna-data-table-actions">
          <Button variant="ghost" type="button" onClick={() => void copy()} disabled={view.length === 0}>
            {copied ? <Check size={14} /> : <Copy size={14} />}{copied ? "已复制" : "复制"}
          </Button>
          <Button variant="ghost" type="button" onClick={exportCsv} disabled={view.length === 0}>
            <Download size={14} />导出 CSV
          </Button>
        </div>
      </div>
      <div className="suna-data-table-scroll">
        <table>
          <thead>
            <tr>
              {columns.map((column) => {
                const active = sort?.columnId === column.id;
                const sortable = column.sortable !== false;
                const Icon = !active ? ArrowUpDown : sort?.direction === "asc" ? ArrowUp : ArrowDown;
                return (
                  <th key={column.id} className={cn(column.align === "right" && "is-right")}>
                    {sortable ? (
                      <button type="button" onClick={() => setSort((current) => toggleSort(current, column.id))} aria-label={`按 ${column.header} 排序`} aria-pressed={active}>
                        {column.header}
                        <Icon size={12} aria-hidden="true" />
                      </button>
                    ) : column.header}
                  </th>
                );
              })}
            </tr>
          </thead>
          <tbody>
            {visible.map((row) => (
              <tr key={rowKey(row)} onClick={onRowClick ? () => onRowClick(row) : undefined} className={onRowClick ? "is-clickable" : undefined}>
                {columns.map((column) => (
                  <td key={column.id} className={cn(column.align === "right" && "is-right")}>
                    {column.render ? column.render(row) : (column.value(row) ?? "—")}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
        {visible.length === 0 && <p className="suna-data-table-empty">{emptyText}</p>}
      </div>
      <div className="suna-data-table-footer">
        <label>
          每页
          <select value={pageSize} onChange={(event) => setPageSize(Number(event.target.value))} aria-label="每页行数">
            {pageSizeOptions.map((size) => <option key={size} value={size}>{size}</option>)}
          </select>
          行
        </label>
        <div className="suna-data-table-pager">
          <Button variant="ghost" type="button" onClick={() => setPage(current - 1)} disabled={current <= 1}>上一页</Button>
          <span>第 {current} / {totalPages} 页</span>
          <Button variant="ghost" type="button" onClick={() => setPage(current + 1)} disabled={current >= totalPages}>下一页</Button>
        </div>
      </div>
    </section>
  );
}
