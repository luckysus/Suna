import { expect, it } from "vitest";
import { applicabilityLabel, applicabilityRow, parseBatchInput } from "./applicability";

it("keeps old object batches in model order", () => {
  expect(parseBatchInput('[{"temperature":900,"C":0.2}]', ["C", "temperature"])).toEqual({ rows: [[0.2, 900]] });
});
it("forwards explicit units and input order to the actual Rust conversion layer", () => {
  expect(parseBatchInput(JSON.stringify({ schema: [{ name: "temperature", unit: "K" }, { name: "C", unit: "fraction" }], rows: [[1123.15, 0.002]] }), ["C", "temperature"])).toEqual({ rows: [[1123.15, 0.002]], inputSchema: [{ name: "temperature", unit: "K" }, { name: "C", unit: "fraction" }] });
});
it("rejects duplicate, missing and unexpected schema fields", () => {
  for (const schema of [[{ name: "C" }, { name: "C" }], [{ name: "C" }], [{ name: "C" }, { name: "other" }]]) {
    expect(() => parseBatchInput(JSON.stringify({ schema, rows: [[1, 2]] }), ["C", "temperature"])).toThrow();
  }
});
it("rejects missing, null and string numerical inputs without silently using zero", () => {
  for (const text of ['[{"C":0.2}]', '[[null,900]]', '[["",900]]', '[["0.2",900]]']) {
    expect(() => parseBatchInput(text, ["C", "temperature"])).toThrow();
  }
});
it("missing legacy risk data remains unknown", () => {
  expect(applicabilityRow(undefined, 0).state).toBe("unknown");
  expect(applicabilityLabel(applicabilityRow(undefined, 0).state)).toBe("适用域未知");
});
it("uses each row's own risk and preserves a missing row as unknown", () => {
  const risk = { support_sha256: null, rows: [{ row_index: 1, state: "outside_domain" as const, reasons: [] }] };
  expect(applicabilityRow(risk, 1).state).toBe("outside_domain");
  expect(applicabilityRow(risk, 0).state).toBe("unknown");
});
