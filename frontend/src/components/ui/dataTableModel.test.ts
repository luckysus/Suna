import { describe, expect, it } from "vitest";
import {
  clampPage,
  compareValues,
  filterRows,
  pageCount,
  slicePage,
  sortRows,
  toCsv,
  toDelimited,
  toggleSort,
} from "./dataTableModel";

describe("compareValues", () => {
  it("orders numbers numerically and strings by locale", () => {
    expect(compareValues(2, 10)).toBeLessThan(0);
    expect(compareValues("b", "a")).toBeGreaterThan(0);
  });

  it("keeps empty values last in both directions", () => {
    expect(compareValues(null, 1)).toBeGreaterThan(0);
    expect(compareValues("", 1)).toBeGreaterThan(0);
    expect(compareValues(null, null)).toBe(0);
  });
});

describe("sortRows", () => {
  const rows = [{ v: 3 }, { v: null }, { v: 1 }, { v: 2 }];

  it("sorts ascending with empty values last", () => {
    expect(sortRows(rows, (row) => row.v, "asc").map((row) => row.v)).toEqual([1, 2, 3, null]);
  });

  it("sorts descending", () => {
    expect(sortRows(rows, (row) => row.v, "desc").map((row) => row.v)).toEqual([3, 2, 1, null]);
  });

  it("does not mutate the input", () => {
    const original = [...rows];
    sortRows(rows, (row) => row.v, "asc");
    expect(rows).toEqual(original);
  });
});

describe("filterRows", () => {
  it("matches case-insensitively and ignores blank queries", () => {
    const rows = [{ name: "Q690" }, { name: "NM450" }];
    expect(filterRows(rows, (row) => row.name, "q6").map((row) => row.name)).toEqual(["Q690"]);
    expect(filterRows(rows, (row) => row.name, "   ")).toHaveLength(2);
  });
});

describe("pagination", () => {
  it("computes page count and clamps the current page", () => {
    expect(pageCount(45, 20)).toBe(3);
    expect(pageCount(0, 20)).toBe(1);
    expect(clampPage(9, 45, 20)).toBe(3);
    expect(clampPage(0, 45, 20)).toBe(1);
  });

  it("slices the requested page", () => {
    const rows = Array.from({ length: 45 }, (_, index) => index);
    expect(slicePage(rows, 2, 20)).toEqual(rows.slice(20, 40));
  });
});

describe("export helpers", () => {
  it("quotes cells containing the delimiter, quotes or newlines", () => {
    const text = toDelimited(["a", "b"], [["x\ty", 'say "hi"']]);
    expect(text).toBe('a\tb\n"x\ty"\t"say ""hi"""');
  });

  it("prefixes CSV with a BOM so Excel reads Chinese", () => {
    const csv = toCsv(["成分", "含量"], [["碳", "0.15"]]);
    expect(csv.startsWith("\uFEFF")).toBe(true);
    expect(csv).toContain("成分,含量\n碳,0.15");
  });
});

describe("toggleSort", () => {
  it("cycles asc → desc → none and restarts on a new column", () => {
    const asc = toggleSort(null, "a");
    expect(asc).toEqual({ columnId: "a", direction: "asc" });
    const desc = toggleSort(asc, "a");
    expect(desc).toEqual({ columnId: "a", direction: "desc" });
    expect(toggleSort(desc, "a")).toBeNull();
    expect(toggleSort(desc, "b")).toEqual({ columnId: "b", direction: "asc" });
  });
});
