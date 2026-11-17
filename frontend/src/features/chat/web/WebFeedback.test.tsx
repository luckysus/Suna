import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import WebFeedback from "./WebFeedback";

describe("WebFeedback", () => {
  it("submits thumbs-up immediately and keeps selection only on success", async () => {
    const onFeedback = vi.fn().mockResolvedValue(true);
    render(<WebFeedback messageId="m1" onFeedback={onFeedback} />);
    fireEvent.click(screen.getByLabelText("有用"));
    await waitFor(() => expect(onFeedback).toHaveBeenCalledWith("up", undefined));
    expect(screen.getByLabelText("有用")).toHaveAttribute("aria-pressed", "true");
  });

  it("writes thumbs-down only after choosing a reason and submitting", async () => {
    const onFeedback = vi.fn().mockResolvedValue(true);
    render(<WebFeedback messageId="m1" onFeedback={onFeedback} />);
    fireEvent.click(screen.getByLabelText("无用"));
    expect(onFeedback).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "内容有误" }));
    fireEvent.click(screen.getByRole("button", { name: "提交" }));
    await waitFor(() => expect(onFeedback).toHaveBeenCalledWith("down", "incorrect"));
  });

  it("shows retry hint when submit fails", async () => {
    const onFeedback = vi.fn().mockResolvedValue(false);
    render(<WebFeedback messageId="m1" onFeedback={onFeedback} />);
    fireEvent.click(screen.getByLabelText("有用"));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("提交失败，请重试"));
    expect(screen.getByLabelText("有用")).toHaveAttribute("aria-pressed", "false");
  });
});
