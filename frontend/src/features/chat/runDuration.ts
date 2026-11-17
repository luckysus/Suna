const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export type RunDurationPart = {
  value: number;
  label: string;
  valueWidthCh: number;
};

/** Chinese duration parts with stable digit width — mirrors steel-agent. */
export function formatRunDurationParts(durationMs: number): {
  label: string;
  parts: RunDurationPart[];
} {
  let remaining = Math.max(SECOND, Math.ceil(durationMs / SECOND) * SECOND);
  const units: Array<[number, string, number]> = [
    [DAY, "天", 2],
    [HOUR, "小时", 2],
    [MINUTE, "分", 2],
    [SECOND, "秒", 2],
  ];
  const parts: RunDurationPart[] = [];
  for (const [unitMs, label, valueWidthCh] of units) {
    const count = Math.floor(remaining / unitMs);
    if (count <= 0) continue;
    parts.push({ value: count, label, valueWidthCh });
    remaining %= unitMs;
  }
  if (parts.length === 0) parts.push({ value: 1, label: "秒", valueWidthCh: 2 });
  return {
    label: parts.map((part) => `${part.value}${part.label}`).join(""),
    parts,
  };
}
