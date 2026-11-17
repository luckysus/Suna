import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SettingsPage from "./SettingsPage";
import { desktop, isDesktopRuntime, type PermissionRuleRecord, type ProviderProfileResponse } from "../../bridge/desktop";
import { ThemeProvider } from "../../theme/theme";
import { AppearanceProvider } from "../../settings/appearance";
import { resetSettingsStoreForTests } from "../../stores/settingsStore";
import { resetMcpStoreForTests } from "../../stores/mcpStore";
import { resetSkillStoreForTests } from "../../stores/skillStore";

vi.mock("../../i18n/locale", () => ({
  useLocale: () => ({
    preference: "zh-CN",
    setPreference: vi.fn(),
    t: (key: string, params?: Record<string, string | number>) =>
      params ? `${key}:${JSON.stringify(params)}` : key,
  }),
}));

vi.mock("../../bridge/desktop", () => ({
  isDesktopRuntime: vi.fn().mockReturnValue(false),
  desktop: {
    setNativeTheme: vi.fn().mockResolvedValue(undefined),
    listProviderProfiles: vi.fn(),
    getSetting: vi.fn(),
    setSetting: vi.fn(),
    saveProviderProfile: vi.fn(),
    setProviderSecret: vi.fn(),
    setDefaultProvider: vi.fn(),
    deleteProviderSecret: vi.fn(),
    deleteProviderProfile: vi.fn(),
    copyProviderProfile: vi.fn(),
    listModelProviderCatalog: vi.fn().mockResolvedValue([]),
    resolveModelCatalog: vi.fn().mockResolvedValue({ cataloged: false }),
    checkOllamaStatus: vi.fn().mockResolvedValue({ reachable: false, base_url: "http://127.0.0.1:11434" }),
    listOllamaModels: vi.fn().mockResolvedValue([]),
    downloadOllamaModel: vi.fn().mockResolvedValue(undefined),
    debugProviderProfile: vi.fn(),
    listenOllamaDownloadProgress: vi.fn().mockResolvedValue(() => undefined),
    listOllamaDownloadTasks: vi.fn().mockResolvedValue([]),
    listProviderCredentialFields: vi.fn().mockResolvedValue([
      { name: "api_key", label: "API Key", secret: true, primary: true, configured: true },
    ]),
    setProviderAuxiliarySecret: vi.fn(),
    deleteProviderAuxiliarySecret: vi.fn(),
    testProviderProfile: vi.fn(),
    listPermissionRules: vi.fn(),
    revokePermissionRule: vi.fn(),
    listDatabaseConnections: vi.fn(),
    saveDatabaseConnection: vi.fn(),
    deleteDatabaseConnection: vi.fn(),
    testDatabaseConnection: vi.fn(),
    listDatabaseTables: vi.fn(),
  },
}));

const chatProfile: ProviderProfileResponse = {
  id: "chat-1",
  kind: "open_ai_compatible",
  display_name: "Steel LLM",
  base_url: "https://api.example.com/v1",
  model_id: "steel-model",
  enabled: true,
  revision: 1,
  secret_generation: 1,
  secret_configured: true,
  model_type: "chat",
  source: "remote",
  status: "active",
  metadata_json: "{}",
  default_for: ["chat"],
};

const embeddingProfile: ProviderProfileResponse = {
  id: "embedding-1",
  kind: "siliconflow",
  display_name: "SiliconFlow Embedding (free)",
  base_url: "https://api.siliconflow.cn/v1",
  model_id: "BAAI/bge-m3",
  enabled: true,
  revision: 1,
  secret_generation: 1,
  secret_configured: true,
  model_type: "embedding",
  source: "remote",
  status: "active",
  metadata_json: "{}",
  default_for: ["embedding"],
};

const rerankerProfile: ProviderProfileResponse = {
  id: "rerank-1",
  kind: "siliconflow",
  display_name: "SiliconFlow Rerank (free)",
  base_url: "https://api.siliconflow.cn/v1",
  model_id: "BAAI/bge-reranker-v2-m3",
  enabled: true,
  revision: 1,
  secret_generation: 1,
  secret_configured: true,
  model_type: "rerank",
  source: "remote",
  status: "active",
  metadata_json: "{}",
  default_for: ["rerank"],
};

async function openModelDrawer(displayName: string) {
  fireEvent.click(await screen.findByRole("tab", { name: "settingsTabProviders" }));
  fireEvent.click(await screen.findByRole("heading", { name: displayName }));
  return await screen.findByDisplayValue(displayName);
}

const permissionRule: PermissionRuleRecord = {
  id: "rule-1",
  tool_id: "builtin.write_file",
  tool_version: { major: 1, minor: 0, patch: 0 },
  source: { kind: "builtin" },
  action: "execute",
  scope: { kind: "exact", value: { path: "draft.txt" } },
  effect: "allow",
};

describe("SettingsPage", () => {
  const renderSettings = () => render(
    <ThemeProvider>
      <AppearanceProvider>
        <SettingsPage />
      </AppearanceProvider>
    </ThemeProvider>,
  );

  beforeEach(() => {
    vi.clearAllMocks();
    resetSettingsStoreForTests();
    resetMcpStoreForTests();
    resetSkillStoreForTests();
    vi.mocked(isDesktopRuntime).mockReturnValue(true);
    vi.mocked(desktop.listDatabaseConnections).mockResolvedValue([]);
    vi.mocked(desktop.listProviderProfiles).mockResolvedValue([chatProfile, embeddingProfile, rerankerProfile]);
    vi.mocked(desktop.getSetting).mockImplementation(async (key) => {
      if (key === "onboarding.completed") return JSON.stringify({ llm_profile_id: chatProfile.id });
      if (key === "onboarding.retrieval") {
        return JSON.stringify({ plan: "free", embedding_profile_id: embeddingProfile.id });
      }
      if (key === "model.provider.preferences") return JSON.stringify({});
      return null;
    });
    vi.mocked(desktop.setDefaultProvider).mockResolvedValue(undefined);
    vi.mocked(desktop.setSetting).mockResolvedValue(undefined);
    vi.mocked(desktop.setProviderSecret).mockResolvedValue({ configured: true });
    vi.mocked(desktop.deleteProviderProfile).mockResolvedValue(undefined);
    vi.mocked(desktop.testProviderProfile).mockResolvedValue({
      ok: true,
      status_code: 200,
      error_code: null,
      elapsed_ms: 12,
    });
    vi.mocked(desktop.listPermissionRules).mockResolvedValue([permissionRule]);
    vi.mocked(desktop.revokePermissionRule).mockResolvedValue(undefined);
    vi.mocked(desktop.saveProviderProfile).mockImplementation(async (input) => ({
      ...chatProfile,
      id: input.id ?? "new-profile",
      kind: input.kind,
      display_name: input.display_name,
      base_url: input.base_url,
      model_id: input.model_id ?? null,
      enabled: input.enabled,
      model_type: input.model_type ?? chatProfile.model_type,
      source: input.source ?? "remote",
      status: input.status ?? "active",
      metadata_json: input.metadata_json ?? "{}",
      default_for: input.id === embeddingProfile.id ? ["embedding"] : input.id === rerankerProfile.id ? ["rerank"] : ["chat"],
    }));
  });

  it("loads provider cards and never displays a configured secret", async () => {
    const { container } = renderSettings();

    expect(await screen.findByRole("heading", { name: "settingsTitle" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "languageLabel" })).toBeInTheDocument();
    await openModelDrawer("Steel LLM");
    expect(screen.getByDisplayValue("https://api.example.com/v1")).toBeInTheDocument();
    expect(screen.getAllByText(/已配置|凭据已配置/).length).toBeGreaterThan(0);
    expect(screen.getByText("Steel LLM", { selector: "h3" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("tab", { name: "settingsCategoryAppearance" }));
    expect(screen.getByRole("group", { name: "themeTitle" })).toBeInTheDocument();
    for (const label of ["themeSystem", "themeLight", "themeDark"]) {
      expect(screen.getByRole("button", { name: label })).toBeInTheDocument();
    }
    fireEvent.click(screen.getByRole("tab", { name: "settingsTabGeneral" }));
    expect(screen.getByText("settingsSecretCopy")).toBeInTheDocument();
    expect(container.querySelectorAll(".suna-eyebrow")).toHaveLength(0);
    expect(screen.queryByText("settingsLede")).not.toBeInTheDocument();
    expect(screen.queryByText("settingsPlanCopy")).not.toBeInTheDocument();
    expect(screen.queryByText("settingsChatDescription")).not.toBeInTheDocument();
    expect(screen.queryByText("settingsEmbeddingDescription")).not.toBeInTheDocument();
    expect(screen.queryByText("settingsRerankerDescription")).not.toBeInTheDocument();
    expect(screen.queryByText("settingsMineruDescription")).not.toBeInTheDocument();
    expect(screen.queryByText("permissionRulesCopy")).not.toBeInTheDocument();
    expect(screen.queryByText("secret-token")).not.toBeInTheDocument();
  });

  it("saves and probes Bailian embedding and reranker from their provider cards", async () => {
    renderSettings();
    for (const [displayName, model, capability] of [
      ["SiliconFlow Embedding (free)", "qwen3.7-text-embedding", "embedding"],
      ["SiliconFlow Rerank (free)", "qwen3.7-text-rerank", "rerank"],
    ] as const) {
      const name = await openModelDrawer(displayName);
      const form = name.closest("form");
      if (!form) throw new Error("provider form is missing");
      fireEvent.click(within(form).getByRole("combobox", { name: "settingsProviderType" }));
      fireEvent.click(await screen.findByRole("option", { name: "providerQwen" }));
      expect(within(form).getByDisplayValue(model)).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "保存并关闭" }));
      await waitFor(() => expect(desktop.saveProviderProfile).toHaveBeenCalledWith(expect.objectContaining({
        kind: "qwen", model_id: model, base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1",
      })));
      const reopened = await openModelDrawer(displayName);
      const retestForm = reopened.closest("form");
      if (!retestForm) throw new Error("provider form is missing");
      fireEvent.click(screen.getByRole("button", { name: "测试连接" }));
      await waitFor(() => expect(desktop.testProviderProfile).toHaveBeenCalledWith(expect.any(String), capability));
      fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    }
  });

  it("persists the selected settings section for the next desktop launch", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("tab", { name: "settingsCategoryAgent" }));

    await waitFor(() => expect(desktop.setSetting).toHaveBeenCalledWith(
      "ui.settings_tab",
      expect.stringContaining('"tab":"agent"'),
    ));
  });

  it("shows empty provider fields when no model has been configured", async () => {
    vi.mocked(desktop.listProviderProfiles).mockResolvedValue([]);
    renderSettings();

    fireEvent.click(await screen.findByRole("tab", { name: "settingsTabProviders" }));
    expect(await screen.findByRole("button", { name: /添加模型/ })).toBeInTheDocument();
    expect(screen.getByText("尚未添加模型，点击「添加模型」开始配置")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /添加模型/ }));
    expect(await screen.findByRole("dialog", { name: /添加模型/ })).toBeInTheDocument();
    expect(screen.getByText(/保存模型后可按字段单独配置/)).toBeInTheDocument();
  });

  it("saves an edited provider and writes a replacement key through the secret bridge", async () => {
    renderSettings();

    const name = await openModelDrawer("Steel LLM");
    fireEvent.change(name, { target: { value: "Updated Steel LLM" } });
    const chatForm = name.closest("form");
    if (!chatForm) throw new Error("chat provider form is missing");
    const keyInput = await screen.findByPlaceholderText("输入新值以替换");
    fireEvent.change(keyInput, { target: { value: "replacement-key" } });
    fireEvent.click(screen.getByRole("button", { name: "保存字段" }));
    fireEvent.click(screen.getByRole("button", { name: "保存并关闭" }));

    await waitFor(() =>
      expect(desktop.saveProviderProfile).toHaveBeenCalledWith(
        expect.objectContaining({
          id: chatProfile.id,
          kind: chatProfile.kind,
          display_name: "Updated Steel LLM",
          base_url: chatProfile.base_url,
          model_id: chatProfile.model_id,
          credential_name: "api_key",
          enabled: true,
          model_type: "chat",
          source: "remote",
          status: "active",
        }),
      ),
    );
    expect(desktop.setProviderSecret).toHaveBeenCalledWith(
      chatProfile.id,
      "api_key",
      "replacement-key",
    );
    expect(desktop.saveProviderProfile).toHaveBeenCalled();
    expect(desktop.setDefaultProvider).toHaveBeenCalledWith("chat", chatProfile.id);
  });

  it("persists the selected chat provider in onboarding state", async () => {
    renderSettings();

    const name = await openModelDrawer("Steel LLM");
    const chatForm = name.closest("form");
    if (!chatForm) throw new Error("chat provider form is missing");
    // Radix Select 不是原生 select：需要先按下指针打开列表，再点选目标项。
    fireEvent.pointerDown(within(chatForm).getByRole("combobox"), { button: 0, ctrlKey: false });
    fireEvent.click(await screen.findByRole("option", { name: "providerDeepSeek" }));
    fireEvent.click(screen.getByRole("button", { name: "保存并关闭" }));

    await waitFor(() =>
      expect(desktop.setSetting).toHaveBeenCalledWith(
        "onboarding.completed",
        expect.stringContaining(`"llm_profile_id":"${chatProfile.id}"`),
      ),
    );
  });

  it("persists the SiliconFlow free or Pro selection without exposing credentials", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("tab", { name: "settingsTabProviders" }));
    fireEvent.click(await screen.findByRole("button", { name: "云端专线" }));
    const pro = await screen.findByLabelText("settingsPlanPro");
    fireEvent.click(pro);

    await waitFor(() =>
      expect(desktop.setSetting).toHaveBeenCalledWith(
        "onboarding.retrieval",
        expect.stringContaining('"plan":"pro"'),
      ),
    );
  });

  it("restores the previous SiliconFlow plan when persistence fails", async () => {
    vi.mocked(desktop.setSetting).mockImplementation(async (key) => {
      if (key === "onboarding.retrieval") throw new Error("settings unavailable");
    });
    renderSettings();

    fireEvent.click(await screen.findByRole("tab", { name: "settingsTabProviders" }));
    fireEvent.click(await screen.findByRole("button", { name: "云端专线" }));
    const free = await screen.findByLabelText("settingsPlanFree");
    fireEvent.click(screen.getByLabelText("settingsPlanPro"));

    await waitFor(() => expect(free).toBeChecked());
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("keeps the edited provider visible when a secret write fails", async () => {
    vi.mocked(desktop.setProviderSecret).mockRejectedValueOnce(new Error("keyring unavailable"));
    renderSettings();

    const name = await openModelDrawer("Steel LLM");
    const chatForm = name.closest("form");
    if (!chatForm) throw new Error("chat provider form is missing");
    fireEvent.change(name, { target: { value: "Unsaved Steel LLM" } });
    const keyInput = await screen.findByPlaceholderText("输入新值以替换");
    fireEvent.change(keyInput, { target: { value: "replacement-key" } });
    fireEvent.click(screen.getByRole("button", { name: "保存字段" }));

    await waitFor(() => expect(screen.getByDisplayValue("Unsaved Steel LLM")).toBeInTheDocument());
    expect(vi.mocked(desktop.listProviderProfiles).mock.calls.length).toBeGreaterThanOrEqual(1);
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("lists persistent permission rules and revokes the selected rule", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("tab", { name: "settingsCategoryAgent" }));
    expect(await screen.findByText("builtin.write_file")).toBeInTheDocument();
    vi.spyOn(window, "confirm").mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: /revoke permission|permissionRevoke/i }));

    await waitFor(() => expect(desktop.revokePermissionRule).toHaveBeenCalledWith("rule-1"));
  });

  it("shows the database panel only on the databases tab", async () => {
    renderSettings();

    expect(screen.queryByRole("heading", { name: "settingsDatabaseTitle" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "settingsTabDatabases" }));
    expect(await screen.findByRole("heading", { name: "settingsDatabaseTitle" })).toBeInTheDocument();
  });

  it("persists appearance controls through the shared preferences setting", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("tab", { name: "settingsCategoryAppearance" }));
    fireEvent.click(screen.getByRole("button", { name: "settingsFontLarge" }));
    fireEvent.click(screen.getByRole("checkbox", { name: /settingsAnimations/ }));

    await waitFor(() => expect(document.documentElement.dataset.fontSize).toBe("large"));

    await waitFor(() => expect(desktop.setSetting).toHaveBeenCalledWith(
      "ui.preferences",
      expect.stringContaining('"fontSize":"large"'),
    ), { timeout: 1500 });
    expect(desktop.setSetting).toHaveBeenCalledWith(
      "ui.preferences",
      expect.stringContaining('"enableAnimations":false'),
    );
  });

  it("applies each font size choice to the document", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("tab", { name: "settingsCategoryAppearance" }));
    const sizes = [
      ["settingsFontSmall", "small"],
      ["settingsFontMedium", "medium"],
      ["settingsFontLarge", "large"],
    ] as const;
    for (const [label, expected] of sizes) {
      fireEvent.click(screen.getByRole("button", { name: label }));
      await waitFor(() => expect(document.documentElement.dataset.fontSize).toBe(expected));
    }
  });

  it("applies each sidebar width choice to the document", async () => {
    renderSettings();

    fireEvent.click(await screen.findByRole("tab", { name: "settingsCategoryAppearance" }));
    const widths = [
      ["settingsSidebarNarrow", "narrow"],
      ["settingsSidebarStandard", "standard"],
      ["settingsSidebarWide", "wide"],
    ] as const;
    for (const [label, expected] of widths) {
      fireEvent.click(screen.getByRole("button", { name: label }));
      await waitFor(() => expect(document.documentElement.dataset.sidebarWidth).toBe(expected));
    }
    await waitFor(() => expect(desktop.setSetting).toHaveBeenCalledWith(
      "ui.preferences",
      expect.stringContaining('"sidebarWidth":"wide"'),
    ), { timeout: 1500 });
  });
});
