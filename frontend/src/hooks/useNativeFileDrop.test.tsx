import { act, render, screen, waitFor } from "@testing-library/react";
import { useRef } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { desktop, type NativeFileDragEvent } from "../bridge/desktop";
import { useNativeFileDrop } from "./useNativeFileDrop";

vi.mock("../bridge/desktop", () => ({ isDesktopRuntime: () => true, desktop: { listenFileDrop: vi.fn() } }));

function Region({ disabled = false, onFiles, onError }: { disabled?: boolean; onFiles: (paths: string[]) => void; onError: (error: unknown) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  useNativeFileDrop(ref, { disabled, onFiles, onError });
  return <div ref={ref}>drop here</div>;
}

describe("native Tauri file drop", () => {
  let listener: (event: NativeFileDragEvent) => void;
  const release = vi.fn();
  beforeEach(() => {
    vi.resetAllMocks();
    vi.mocked(desktop.listenFileDrop).mockImplementation(async (callback) => { listener = callback; return release; });
    vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ left: 10, right: 110, top: 10, bottom: 110 } as DOMRect);
    Object.defineProperty(window, "devicePixelRatio", { value: 2, configurable: true });
  });

  it("uses native paths, scales coordinates and ignores drops outside the region", async () => {
    const onFiles = vi.fn();
    const view = render(<Region onFiles={onFiles} onError={vi.fn()} />);
    await waitFor(() => expect(desktop.listenFileDrop).toHaveBeenCalledOnce());
    await act(async () => listener({ type: "drop", paths: ["D:/中文 数据.csv"], position: { x: 100, y: 100 } } as NativeFileDragEvent));
    expect(onFiles).toHaveBeenCalledWith(["D:/中文 数据.csv"]);
    await act(async () => listener({ type: "drop", paths: ["D:/outside.csv"], position: { x: 300, y: 300 } } as NativeFileDragEvent));
    expect(onFiles).toHaveBeenCalledOnce();
    view.unmount();
    expect(release).toHaveBeenCalledOnce();
  });

  it("uses the current disabled state", async () => {
    const onFiles = vi.fn();
    const onError = vi.fn();
    const view = render(<Region onFiles={onFiles} onError={onError} />);
    view.rerender(<Region disabled onFiles={onFiles} onError={onError} />);
    await act(async () => listener({ type: "drop", paths: ["D:/data.csv"], position: { x: 100, y: 100 } } as NativeFileDragEvent));
    expect(onFiles).not.toHaveBeenCalled();
    expect(screen.getByText("drop here")).toBeInTheDocument();
    view.unmount();
  });

  it("releases a subscription that resolves after unmount", async () => {
    let resolve!: (release: () => void) => void;
    vi.mocked(desktop.listenFileDrop).mockImplementation(() => new Promise((done) => { resolve = done; }));
    const onFiles = vi.fn();
    const view = render(<Region onFiles={onFiles} onError={vi.fn()} />);
    view.unmount();
    await act(async () => resolve(release));
    expect(release).toHaveBeenCalledOnce();
    expect(onFiles).not.toHaveBeenCalled();
  });
});
