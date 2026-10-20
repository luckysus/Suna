import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import SettingsProviderCard from "./SettingsProviderCard";
import type { SettingsEditor } from "./settingsModel";

vi.mock("../../i18n/locale", () => ({
  useLocale: () => ({
    preference: "zh-CN",
    setPreference: vi.fn(),
    t: (key: string) => key,
  }),
}));

function editor(partial: Partial<SettingsEditor> & Pick<SettingsEditor, "slot" | "kind">): SettingsEditor {
  return {
    id: null,
    displayName: "Test",
    baseUrl: "http://127.0.0.1:11434",
    modelId: "llama3",
    apiKey: "",
    enabled: true,
    secretConfigured: false,
    temperature: 0.2,
    maxTokens: 2048,
    ...partial,
  };
}

describe("SettingsProviderCard catalog kinds", () => {
  it("renders embedding + ollama without crashing (catalog click)", () => {
    render(
      <SettingsProviderCard
        editor={editor({ slot: "embedding", kind: "ollama" })}
        busy={false}
        testing={false}
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        onTest={vi.fn()}
        onDelete={vi.fn()}
        hideApiKey
        hideCardActions
        hideDelete
        hideEnabledToggle
      />,
    );
    expect(screen.getByLabelText("settingsProviderType")).toBeInTheDocument();
  });

  it("renders chat + siliconflow without crashing (catalog click)", () => {
    render(
      <SettingsProviderCard
        editor={editor({
          slot: "chat",
          kind: "siliconflow",
          baseUrl: "https://api.siliconflow.cn/v1",
          modelId: "deepseek-ai/DeepSeek-V3",
        })}
        busy={false}
        testing={false}
        onChange={vi.fn()}
        onSubmit={vi.fn()}
        onTest={vi.fn()}
        onDelete={vi.fn()}
        hideApiKey
        hideCardActions
        hideDelete
        hideEnabledToggle
      />,
    );
    expect(screen.getByLabelText("settingsProviderType")).toBeInTheDocument();
  });
});
