import { describe, expect, it } from "vitest";
import { defaultShortcuts, normalizeShortcuts, parseShortcut, shortcutChoices, shortcutRows } from "./shortcuts";

describe("shortcut defaults", () => {
  it("covers the bindings required by the product doc", () => {
    expect(defaultShortcuts.newConversation).toBe("Ctrl+N");
    expect(defaultShortcuts.openCommand).toBe("Ctrl+K");
    expect(defaultShortcuts.openKnowledge).toBe("Ctrl+Shift+K");
    expect(defaultShortcuts.send).toBe("Enter");
    expect(defaultShortcuts.stopAgent).toBe("Esc");
  });

  it("exposes every default binding as a selectable choice", () => {
    for (const binding of Object.values(defaultShortcuts)) {
      expect(shortcutChoices).toContain(binding);
    }
    expect(shortcutRows).toHaveLength(Object.keys(defaultShortcuts).length);
  });
});

describe("normalizeShortcuts", () => {
  it("falls back to defaults for missing or malformed values", () => {
    expect(normalizeShortcuts(null).openKnowledge).toBe("Ctrl+Shift+K");
    expect(normalizeShortcuts("not json").openKnowledge).toBe("Ctrl+Shift+K");
    expect(normalizeShortcuts(null).send).toBe("Enter");
  });

  it("keeps persisted overrides and fills the rest from the fallback", () => {
    const merged = normalizeShortcuts(JSON.stringify({ version: 1, openKnowledge: "Ctrl+Shift+L" }));
    expect(merged.openKnowledge).toBe("Ctrl+Shift+L");
    expect(merged.openCommand).toBe("Ctrl+K");
  });

  it("migrates the old Ctrl+Enter send default to Enter before schema v2", () => {
    const migrated = normalizeShortcuts(JSON.stringify({ version: 1, send: "Ctrl+Enter" }));
    expect(migrated.send).toBe("Enter");
  });

  it("keeps an explicit Ctrl+Enter send binding once schema is v2+", () => {
    const kept = normalizeShortcuts(JSON.stringify({ version: 2, send: "Ctrl+Enter" }));
    expect(kept.send).toBe("Ctrl+Enter");
  });
});

describe("parseShortcut", () => {
  it("parses modifiers and key", () => {
    expect(parseShortcut("Ctrl+Shift+K")).toEqual({ key: "K", ctrl: true, shift: true, alt: false, meta: false });
    expect(parseShortcut("Esc")).toEqual({ key: "Esc", ctrl: false, shift: false, alt: false, meta: false });
    expect(parseShortcut("Ctrl+Enter")).toEqual({ key: "Enter", ctrl: true, shift: false, alt: false, meta: false });
  });
});
