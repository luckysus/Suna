import { describe, expect, it } from "vitest";
import { formatRunDurationParts } from "./runDuration";

describe("formatRunDurationParts", () => {
  it("floors sub-second runs to 1 second", () => {
    expect(formatRunDurationParts(120).label).toBe("1秒");
  });

  it("formats minutes and seconds like the web timer", () => {
    expect(formatRunDurationParts(65_000).label).toBe("1分5秒");
  });
});
