import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useDebouncedValue } from "./useDebouncedValue";
import { useDebouncedCallback } from "./useDebouncedCallback";

beforeEach(() => { vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); });

describe("useDebouncedValue", () => {
  it("keeps the previous value until the delay elapses", () => {
    const { result, rerender } = renderHook(
      ({ value }: { value: string }) => useDebouncedValue(value, 100),
      { initialProps: { value: "a" } },
    );
    expect(result.current).toBe("a");

    rerender({ value: "b" });
    rerender({ value: "c" });
    expect(result.current).toBe("a");

    act(() => { vi.advanceTimersByTime(100); });
    expect(result.current).toBe("c");
  });
});

describe("useDebouncedCallback", () => {
  it("coalesces rapid calls into the last one", () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebouncedCallback(callback, 100));
    act(() => { result.current.schedule(1); result.current.schedule(2); });
    expect(callback).not.toHaveBeenCalled();
    act(() => { vi.advanceTimersByTime(100); });
    expect(callback).toHaveBeenCalledTimes(1);
    expect(callback).toHaveBeenCalledWith(2);
  });

  it("cancel drops the pending call", () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebouncedCallback(callback, 100));
    act(() => { result.current.schedule("x"); result.current.cancel(); });
    act(() => { vi.advanceTimersByTime(300); });
    expect(callback).not.toHaveBeenCalled();
  });

  it("flush runs the callback immediately", () => {
    const callback = vi.fn();
    const { result } = renderHook(() => useDebouncedCallback(callback, 100));
    act(() => { result.current.flush("now"); });
    expect(callback).toHaveBeenCalledWith("now");
  });

  it("drops the pending call on unmount", () => {
    const callback = vi.fn();
    const { result, unmount } = renderHook(() => useDebouncedCallback(callback, 100));
    act(() => { result.current.schedule("x"); });
    unmount();
    act(() => { vi.advanceTimersByTime(300); });
    expect(callback).not.toHaveBeenCalled();
  });
});
