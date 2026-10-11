import { describe, expect, it } from "vitest";
import { defaultAppearancePreferences, normalizePreferences } from "./appearanceModel";

describe("appearanceModel", () => {
  it("returns defaults for non-object input", () => {
    expect(normalizePreferences(null)).toEqual(defaultAppearancePreferences);
    expect(normalizePreferences("x")).toEqual(defaultAppearancePreferences);
  });

  it("keeps valid enum preferences and defaults the rest", () => {
    const result = normalizePreferences({
      fontSize: "large",
      density: "compact",
      startupPage: "knowledge",
      sidebarWidth: "wide",
    });
    expect(result.fontSize).toBe("large");
    expect(result.density).toBe("compact");
    expect(result.startupPage).toBe("knowledge");
    expect(result.sidebarWidth).toBe("wide");
    expect(result.enableAnimations).toBe(true);
  });

  it("maps legacy reduceMotion to enableAnimations=false", () => {
    expect(normalizePreferences({ reduceMotion: true }).enableAnimations).toBe(false);
    expect(normalizePreferences({ reduceMotion: false }).enableAnimations).toBe(true);
    expect(normalizePreferences({ enableAnimations: false }).enableAnimations).toBe(false);
  });
});
