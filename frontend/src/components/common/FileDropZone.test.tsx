import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FileDropZone } from "./FileDropZone";

function fileWithPath(name: string, path: string): File {
  const file = new File(["data"], name, { type: "text/plain" });
  Object.defineProperty(file, "path", { value: path });
  return file;
}

function drop(element: HTMLElement, files: File[]) {
  fireEvent.drop(element, { dataTransfer: { files } });
}

describe("FileDropZone", () => {
  it("passes accepted paths to onFiles", async () => {
    const onFiles = vi.fn();
    render(
      <FileDropZone extensions={["csv", "xlsx"]} onFiles={onFiles}>
        <span>内容</span>
      </FileDropZone>,
    );
    drop(screen.getByText("内容").parentElement as HTMLElement, [fileWithPath("a.csv", "/tmp/a.csv")]);
    expect(onFiles).toHaveBeenCalledWith(["/tmp/a.csv"]);
  });

  it("rejects unsupported extensions without calling onFiles", () => {
    const onFiles = vi.fn();
    const onRejected = vi.fn();
    render(
      <FileDropZone extensions={["csv"]} onFiles={onFiles} onRejected={onRejected}>
        <span>内容</span>
      </FileDropZone>,
    );
    drop(screen.getByText("内容").parentElement as HTMLElement, [fileWithPath("a.exe", "/tmp/a.exe")]);
    expect(onFiles).not.toHaveBeenCalled();
    expect(onRejected).toHaveBeenCalled();
  });

  it("reports when the runtime cannot provide a path", () => {
    const onFiles = vi.fn();
    const onRejected = vi.fn();
    render(
      <FileDropZone extensions={["csv"]} onFiles={onFiles} onRejected={onRejected}>
        <span>内容</span>
      </FileDropZone>,
    );
    drop(screen.getByText("内容").parentElement as HTMLElement, [new File(["x"], "a.csv")]);
    expect(onFiles).not.toHaveBeenCalled();
    expect(onRejected).toHaveBeenCalledWith(expect.stringContaining("无法读取拖拽文件路径"), []);
  });

  it("ignores drops while disabled", () => {
    const onFiles = vi.fn();
    render(
      <FileDropZone extensions={["csv"]} onFiles={onFiles} disabled>
        <span>内容</span>
      </FileDropZone>,
    );
    drop(screen.getByText("内容").parentElement as HTMLElement, [fileWithPath("a.csv", "/tmp/a.csv")]);
    expect(onFiles).not.toHaveBeenCalled();
  });

  it("shows the overlay while dragging over", () => {
    render(
      <FileDropZone extensions={["csv"]} onFiles={vi.fn()} label="松开即可导入数据集">
        <span>内容</span>
      </FileDropZone>,
    );
    fireEvent.dragOver(screen.getByText("内容").parentElement as HTMLElement);
    expect(screen.getByText("松开即可导入数据集")).toBeInTheDocument();
  });
});
