import type { DatasetColumnAnalysis, DatasetCorrelation } from "../../bridge/desktop";

/**
 * 相关性热力图（第 32 章）。
 *
 * 使用后端 `analyze_steel_dataset` 已算好的 Pearson 相关系数渲染，
 * 不自行计算统计量。颜色按中文科研习惯：正相关偏蓝、负相关偏红，
 * 深浅表示强度绝对值。
 */
export default function CorrelationHeatmap({
  correlations,
  columns,
}: {
  correlations: DatasetCorrelation[];
  columns: DatasetColumnAnalysis[];
}) {
  const numeric = columns.filter((column) => column.inferredType === "number");
  if (correlations.length === 0 || numeric.length < 2) {
    return (
      <div className="suna-data-empty">
        <strong>暂无可用的相关性结果</strong>
        <span>至少需要两个数值字段；分析时会自动选取数值列计算 Pearson 相关系数。</span>
      </div>
    );
  }

  const indexOf = new Map(numeric.map((column) => [column.ordinal, column]));
  const lookup = new Map<string, number>();
  for (const item of correlations) {
    if (item.pearson === null) continue;
    lookup.set(`${item.leftOrdinal}:${item.rightOrdinal}`, item.pearson);
    lookup.set(`${item.rightOrdinal}:${item.leftOrdinal}`, item.pearson);
  }

  const tone = (value: number) => {
    const alpha = Math.min(0.88, Math.max(0.08, Math.abs(value) * 0.88));
    return value >= 0
      ? `rgba(var(--suna-text-rgb), ${alpha.toFixed(2)})`
      : `rgba(128, 128, 128, ${(alpha * 0.5).toFixed(2)})`;
  };

  const cellColor = (value: number | undefined) => {
    if (value === undefined) return "var(--suna-text-muted)";
    if (value >= 0 && Math.abs(value) > 0.45) return "var(--suna-bg)";
    return "var(--suna-text)";
  };

  return (
    <div className="suna-correlation">
      <div
        className="suna-correlation-grid"
        style={{ gridTemplateColumns: `minmax(72px, auto) repeat(${numeric.length}, minmax(34px, 1fr))` }}
        role="table"
        aria-label="字段相关性热力图"
      >
        <span className="suna-correlation-corner" aria-hidden="true" />
        {numeric.map((column) => (
          <span className="suna-correlation-head" key={`head-${column.ordinal}`} title={column.name}>
            {column.name}
          </span>
        ))}
        {numeric.map((row) => (
          <span className="suna-correlation-row" key={`row-${row.ordinal}`}>
            <span className="suna-correlation-head is-row" title={row.name}>{row.name}</span>
            {numeric.map((column) => {
              const value = row.ordinal === column.ordinal ? 1 : lookup.get(`${row.ordinal}:${column.ordinal}`);
              return (
                <span
                  className="suna-correlation-cell"
                  key={`${row.ordinal}-${column.ordinal}`}
                  style={{
                    background: value === undefined ? "transparent" : tone(value),
                    color: cellColor(value),
                  }}
                  title={value === undefined
                    ? `${row.name} × ${column.name}：样本不足`
                    : `${row.name} × ${column.name}：r = ${value.toFixed(3)}`}
                >
                  {value === undefined ? "—" : value.toFixed(2)}
                </span>
              );
            })}
          </span>
        ))}
      </div>
      <p className="suna-correlation-legend">
        <span>负相关</span>
        <i style={{ background: tone(-1) }} />
        <i style={{ background: tone(-0.5) }} />
        <i style={{ background: "transparent", border: "0.5px solid var(--suna-line)" }} />
        <i style={{ background: tone(0.5) }} />
        <i style={{ background: tone(1) }} />
        <span>正相关</span>
      </p>
    </div>
  );
}
