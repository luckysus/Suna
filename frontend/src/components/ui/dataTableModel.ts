/**
 * 第 67 章：科研数据表格的纯逻辑（排序 / 筛选 / 分页 / 复制 / 导出）。
 *
 * 与渲染分离，便于单测覆盖。
 */

export type SortDirection = "asc" | "desc";

export type SortState = { columnId: string; direction: SortDirection };

/** 数值按大小比较，其余按本地化字符串比较；空值恒排在后。 */
export function compareValues(left: string | number | null, right: string | number | null): number {
  const leftEmpty = left === null || left === "";
  const rightEmpty = right === null || right === "";
  if (leftEmpty && rightEmpty) return 0;
  if (leftEmpty) return 1;
  if (rightEmpty) return -1;
  if (typeof left === "number" && typeof right === "number") return left - right;
  return String(left).localeCompare(String(right), "zh-CN", { numeric: true });
}

export function sortRows<T>(rows: T[], value: (row: T) => string | number | null, direction: SortDirection): T[] {
  const factor = direction === "asc" ? 1 : -1;
  return [...rows].sort((left, right) => {
    const leftValue = value(left);
    const rightValue = value(right);
    const leftEmpty = leftValue === null || leftValue === "";
    const rightEmpty = rightValue === null || rightValue === "";
    // 空值不受方向影响，始终排在末尾。
    if (leftEmpty || rightEmpty) return leftEmpty && rightEmpty ? 0 : leftEmpty ? 1 : -1;
    return factor * compareValues(leftValue, rightValue);
  });
}

export function filterRows<T>(rows: T[], text: (row: T) => string, query: string): T[] {
  const needle = query.trim().toLocaleLowerCase();
  if (!needle) return rows;
  return rows.filter((row) => text(row).toLocaleLowerCase().includes(needle));
}

export function pageCount(total: number, pageSize: number): number {
  if (pageSize <= 0) return 1;
  return Math.max(1, Math.ceil(total / pageSize));
}

export function clampPage(page: number, total: number, pageSize: number): number {
  return Math.min(Math.max(1, page), pageCount(total, pageSize));
}

export function slicePage<T>(rows: T[], page: number, pageSize: number): T[] {
  if (pageSize <= 0) return rows;
  const start = (clampPage(page, rows.length, pageSize) - 1) * pageSize;
  return rows.slice(start, start + pageSize);
}

function escapeCell(value: string | number | null, delimiter: string): string {
  const text = value === null ? "" : String(value);
  if (text.includes(delimiter) || text.includes("\n") || text.includes('"')) {
    return `"${text.replace(/"/g, '""')}"`;
  }
  return text;
}

/** 制表符分隔文本，便于直接粘贴进 Excel / Word 表格。 */
export function toDelimited(headers: string[], rows: Array<Array<string | number | null>>, delimiter = "\t"): string {
  return [headers, ...rows].map((row) => row.map((cell) => escapeCell(cell, delimiter)).join(delimiter)).join("\n");
}

/** 带 BOM 的 CSV，保证 Excel 正确识别中文。 */
export function toCsv(headers: string[], rows: Array<Array<string | number | null>>): string {
  return `\uFEFF${toDelimited(headers, rows, ",")}`;
}

/** 切换某一列的排序方向：同列升序→降序→取消，换列则从升序开始。 */
export function toggleSort(current: SortState | null, columnId: string): SortState | null {
  if (current?.columnId !== columnId) return { columnId, direction: "asc" };
  if (current.direction === "asc") return { columnId, direction: "desc" };
  return null;
}
