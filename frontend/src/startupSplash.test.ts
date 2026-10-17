import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import indexHtml from "../index.html?raw";
import { dismissBootScreen } from "./startupSplash";

describe("startup shell", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
    document.body.innerHTML = "";
  });

  it("keeps the Tauri window non-blank before React mounts", () => {
    expect(indexHtml).toContain('id="suna-boot-screen"');
    expect(indexHtml).toContain("SUNA");
  });

  it("dismisses boot screen with smooth leaving animation and removes from DOM", () => {
    const el = document.createElement("div");
    el.id = "suna-boot-screen";
    document.body.appendChild(el);

    dismissBootScreen();
    expect(el.classList.contains("is-leaving")).toBe(true);

    vi.advanceTimersByTime(500);
    expect(document.getElementById("suna-boot-screen")).toBeNull();
  });
});
