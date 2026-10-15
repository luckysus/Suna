import { Sparkles } from "lucide-react";
import type { WebRecommendation } from "./webTypes";

function valueText(value: unknown) {
  if (value === null || value === undefined || value === "") return "-";
  if (typeof value === "number") return Number.isInteger(value) ? String(value) : value.toFixed(3).replace(/0+$/, "").replace(/\.$/, "");
  return typeof value === "string" ? value : JSON.stringify(value);
}

export default function WebRecommendationCard({ item }: { item: WebRecommendation }) {
  const entries = Object.entries(item.details ?? {}).filter(([, value]) => value !== null && value !== undefined && value !== "").slice(0, 8);
  return (
    <div className="rounded-xl border border-[var(--suna-line)] bg-[var(--suna-bg-raised)] p-4 shadow-sm">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h4 className="text-base font-bold text-[var(--suna-text)]">{item.title}</h4>
          {item.category && <div className="mt-0.5 text-xs font-semibold uppercase tracking-wide text-[var(--suna-text-muted)]">{item.category}</div>}
        </div>
        <Sparkles size={16} className="mt-1 shrink-0 text-[var(--suna-text)]" />
      </div>
      {item.summary && <p className="mt-2 text-base leading-relaxed text-[var(--suna-text-soft)]">{item.summary}</p>}
      {entries.length > 0 && (
        <div className="mt-3 grid grid-cols-2 gap-2 sm:grid-cols-3 lg:grid-cols-4">
          {entries.map(([key, value]) => (
            <div key={key} className="rounded-lg border border-[var(--suna-line)] bg-[var(--suna-bg-soft)] px-3 py-2">
              <div className="text-xs font-semibold text-[var(--suna-text-muted)]">{key}</div>
              <div className="mt-0.5 break-words text-sm font-semibold text-[var(--suna-text)]">{valueText(value)}</div>
            </div>
          ))}
        </div>
      )}
      {(item.risks ?? []).length > 0 && <div className="mt-3 rounded-lg border border-[var(--suna-line-strong)] bg-[var(--suna-bg-soft)] px-3 py-2 text-sm leading-relaxed text-[var(--suna-text)]">{item.risks?.slice(0, 2).join("；")}</div>}
    </div>
  );
}
