import { describe, expect, it } from "vitest";
import { describeError, errorMessage, redactErrorMessage } from "./errorMessage";

describe("redactErrorMessage", () => {
  it("hides credentials and low-level diagnostics", () => {
    expect(redactErrorMessage(new Error("api_key: sk-live-secret"))).toBe("");
    expect(redactErrorMessage(new Error("postgresql://postgres:secret@127.0.0.1:5432/suna"))).toBe("");
    expect(redactErrorMessage(new Error("SQLSTATE 28P01: password authentication failed"))).toBe("");
    expect(redactErrorMessage(new Error("panicked at src/main.rs:42"))).toBe("");
  });

  it("keeps a short user-facing message", () => {
    expect(redactErrorMessage(new Error("连接超时，请检查服务是否运行"))).toBe("连接超时，请检查服务是否运行");
  });

  it("returns an empty string for non-error values", () => {
    expect(redactErrorMessage({ reason: "unknown" })).toBe("");
  });
});

describe("errorMessage", () => {
  it("falls back when nothing safe can be shown", () => {
    expect(errorMessage(new Error("password=super-secret"), "设置操作失败，请重试")).toBe("设置操作失败，请重试");
  });

  it("passes through safe messages", () => {
    expect(errorMessage(new Error("模型未配置"), "失败")).toBe("模型未配置");
  });
});

describe("describeError", () => {
  it("classifies connection failures", () => {
    const result = describeError(new Error("ECONNREFUSED 127.0.0.1:11434"));
    expect(result.category).toBe("connection");
    expect(result.title).toBe("模型连接失败");
    expect(result.retryable).toBe(true);
  });

  it("classifies data format failures as non-retryable", () => {
    const result = describeError(new Error("invalid json payload"));
    expect(result.category).toBe("data");
    expect(result.title).toBe("数据格式错误");
    expect(result.retryable).toBe(false);
  });

  it("falls back to a generic title with the provided detail", () => {
    const result = describeError(new Error(""), "任务执行失败，请重试");
    expect(result.category).toBe("unknown");
    expect(result.title).toBe("任务执行失败");
    expect(result.detail).toBe("任务执行失败，请重试");
  });
});
