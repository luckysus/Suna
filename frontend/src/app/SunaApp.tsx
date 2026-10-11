import { useEffect, useMemo, useRef, useState } from "react";
import { ChevronRight, House, LayoutGrid, MessageSquareText, Plus, Search, Settings2, Sun } from "lucide-react";
import { desktop, isDesktopRuntime } from "../bridge/desktop";
import ChatPage from "../features/chat/ChatPage";
import { ChatControllerProvider } from "../features/chat/chatController";
import { useChatControllerContext } from "../features/chat/chatController";
import DiagnosticsPage from "../features/diagnostics/DiagnosticsPage";
import KnowledgeCenterPage from "../features/knowledge/KnowledgeCenterPage";
import ResearchModulePage from "../features/research/ResearchModulePage";
import LiteratureResearchPage from "../features/literature/LiteratureResearchPage";
import PerformancePredictionPage from "../features/prediction/PerformancePredictionPage";
import ProcessOptimizationPage from "../features/optimization/ProcessOptimizationPage";
import McpManagementPage from "../features/mcp/McpManagementPage";
import ExperimentAssistantPage from "../features/experiment/ExperimentAssistantPage";
import ResearchReportPage from "../features/reports/ResearchReportPage";
import DataLabPage from "../features/data/DataLabPage";
import CapabilityManagementPage from "../features/management/CapabilityManagementPage";
import SettingsPage from "../features/settings/SettingsPage";
import { getNavigationSection, navigationSections, primaryNavigationSections, utilityNavigationSections, type NavigationSection, type SectionId } from "./navigation";
import { isRootPath, useSectionRoute } from "./sectionRoute";
import { defaultShortcuts, normalizeShortcuts, parseShortcut, type ShortcutSettings } from "./shortcuts";
import { filterGlobalEntities, loadGlobalEntities, searchLiteratureItems, type GlobalEntity } from "./globalSearch";
import { CommandMenu, type CommandMenuGroup, type CommandMenuItem } from "../components/ui/command-menu";
import { ToastProvider, useToast } from "../components/ui/toast";
import { LocaleProvider } from "../i18n/locale";
import { ThemeProvider, useTheme } from "../theme/theme";
import { AppearanceProvider, useAppearanceSettings } from "../settings/appearance";
import SunaLogo from "../components/SunaLogo";
import "./suna-shell.css";

export default function SunaApp() { return <ToastProvider><LocaleProvider><ThemeProvider><AppearanceProvider><ChatControllerProvider><SunaAppShell /></ChatControllerProvider></AppearanceProvider></ThemeProvider></LocaleProvider></ToastProvider>; }
function SunaMark() { return <SunaLogo size={37} title="Suna" aria-hidden="true" />; }
function ModuleView({ section, onOpenSettings }: { section: SectionId; onOpenSettings: () => void }) { return section === "literature" ? <LiteratureResearchPage /> : section === "prediction" ? <PerformancePredictionPage /> : section === "optimization" ? <ProcessOptimizationPage /> : section === "mcp" ? <McpManagementPage /> : section === "experiment" ? <ExperimentAssistantPage /> : section === "reports" ? <ResearchReportPage /> : section === "data" ? <DataLabPage /> : section === "agents" || section === "tools" || section === "skills" ? <CapabilityManagementPage section={section} /> : section === "models" ? <SettingsPage onOpenDiagnostics={onOpenSettings} initialTab="providers" /> : section === "account" ? <SettingsPage onOpenDiagnostics={onOpenSettings} initialTab="account" /> : section === "about" ? <SettingsPage onOpenDiagnostics={onOpenSettings} initialTab="about" /> : <ResearchModulePage section={section} />; }
function NavButton({ item, active, onClick }: { item: NavigationSection; active: SectionId; onClick: (id: SectionId) => void }) { const Icon = item.icon; const label = item.id === "chat" ? "对话中心" : item.label; return <button className={`suna-new-nav-item ${active === item.id ? "is-active" : ""}`} onClick={() => onClick(item.id)}><Icon size={17} /><span>{label}</span>{active === item.id && <span className="suna-new-nav-indicator" />}</button>; }
function SunaAppShell() {
  const chat = useChatControllerContext();
  const toast = useToast();
  const { preferences, loaded: appearanceLoaded, updatePreferences } = useAppearanceSettings();
  const { resolvedTheme, setPreference: setThemePreference } = useTheme();
  const startupApplied = useRef(false);
  const updateCheckStarted = useRef(false);
  const [activeSection, setActiveSection] = useSectionRoute();
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Awaited<ReturnType<typeof chat.onSearchHistory>>>([]);
  const [entities, setEntities] = useState<GlobalEntity[]>([]);
  const [literatureItems, setLiteratureItems] = useState<CommandMenuItem[]>([]);
  const [modelOpen, setModelOpen] = useState(false);
  const [accountName, setAccountName] = useState("Suna");
  const [shortcuts, setShortcuts] = useState<ShortcutSettings>(defaultShortcuts);
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  useEffect(() => {
    if (!appearanceLoaded || startupApplied.current) return;
    startupApplied.current = true;
    if (preferences.startupPage !== "chat" && isRootPath(window.location.pathname)) setActiveSection(preferences.startupPage);
  }, [appearanceLoaded, preferences.startupPage]);
  useEffect(() => {
    if (!appearanceLoaded || !preferences.autoUpdate || !isDesktopRuntime() || updateCheckStarted.current) return;
    updateCheckStarted.current = true;
    void desktop.checkForUpdates().then((result) => {
      if (result.update_available) {
        toast.push({ tone: "info", title: `发现新版本 ${result.latest_version}`, description: "可在「设置 → 关于」查看发布页。" });
      }
    }).catch(() => undefined);
  }, [appearanceLoaded, preferences.autoUpdate]);
  useEffect(() => {
    document.querySelector<HTMLElement>(".suna-new-content")?.scrollTo({ top: 0, behavior: "auto" });
  }, [activeSection]);
  const searchRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    void desktop.getSetting("profile.account").then((value) => {
      try {
        const parsed = JSON.parse(value ?? "{}") as { display_name?: unknown };
        if (typeof parsed.display_name === "string" && parsed.display_name.trim()) setAccountName(parsed.display_name.trim());
      } catch { /* optional profile setting */ }
    }).catch(() => undefined);
  }, []);
  useEffect(() => {
    const changed = (event: Event) => {
      const detail = (event as CustomEvent<{ key?: string; value?: string }>).detail;
      if (!detail?.key || !detail.value) return;
      try {
        const parsed = JSON.parse(detail.value) as Record<string, unknown>;
        if (detail.key === "profile.account" && typeof parsed.display_name === "string" && parsed.display_name.trim()) {
          setAccountName(parsed.display_name.trim());
        }
        if (detail.key === "ui.shortcuts") {
          setShortcuts((current) => normalizeShortcuts(detail.value ?? null, current));
        }
      } catch { /* keep the current shell state for malformed imports */ }
    };
    window.addEventListener("suna:setting-changed", changed);
    return () => window.removeEventListener("suna:setting-changed", changed);
  }, []);

  useEffect(() => {
    void desktop.getSetting("ui.shortcuts").then((raw) => {
      setShortcuts((current) => normalizeShortcuts(raw, current));
    }).catch(() => undefined);
  }, []);
  useEffect(() => { if (searchOpen) searchRef.current?.focus(); }, [searchOpen]);
  // 第 72 章：静态实体（知识库 / Agent / Skill / MCP / 模型）加载一次，之后本地过滤。
  useEffect(() => { void loadGlobalEntities().then(setEntities).catch(() => undefined); }, []);
  useEffect(() => {
    if (!searchOpen || !searchQuery.trim()) { setSearchResults([]); setLiteratureItems([]); return; }
    const timer = window.setTimeout(() => {
      void chat.onSearchHistory(searchQuery).then(setSearchResults);
      void searchLiteratureItems(searchQuery).then(setLiteratureItems);
    }, 180);
    return () => window.clearTimeout(timer);
  }, [chat.onSearchHistory, searchOpen, searchQuery]);
  useEffect(() => {
    const matches = (event: KeyboardEvent, binding?: string) => {
      if (!binding || binding === "未设置") return false;
      const { key, ctrl, shift, alt, meta } = parseShortcut(binding);
      const actualKey = event.key === "Escape" ? "esc" : event.key.toLowerCase();
      return actualKey === key.toLowerCase() && event.ctrlKey === ctrl && event.shiftKey === shift && event.altKey === alt && event.metaKey === meta;
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (matches(event, shortcuts.openKnowledge)) { event.preventDefault(); setActiveSection("knowledge"); return; }
      if (matches(event, shortcuts.openCommand) || matches(event, shortcuts.searchHistory)) { event.preventDefault(); setSearchOpen(true); return; }
      if (matches(event, shortcuts.newConversation)) { event.preventDefault(); void chat.onNewConversation(); setActiveSection("chat"); return; }
      if (matches(event, shortcuts.openSettings)) { event.preventDefault(); setActiveSection("settings"); return; }
      if (matches(event, shortcuts.focusInput)) { event.preventDefault(); (document.querySelector("textarea") as HTMLTextAreaElement | null)?.focus(); return; }
      if (matches(event, shortcuts.stopAgent)) { event.preventDefault(); chat.onCancel(); return; }
      if (matches(event, shortcuts.toggleSidebar)) { event.preventDefault(); setSidebarCollapsed((collapsed) => !collapsed); return; }
      if (matches(event, shortcuts.toggleInspector)) { event.preventDefault(); updatePreferences({ showAgentPanel: !preferences.showAgentPanel }); return; }
      if (matches(event, shortcuts.recoverTask)) { event.preventDefault(); chat.onResume(); return; }
      if (event.key === "Escape") { setSearchOpen(false); setModelOpen(false); }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [chat.onCancel, chat.onNewConversation, chat.onResume, preferences.showAgentPanel, shortcuts, updatePreferences]);
  const active = getNavigationSection(activeSection);
  const activeProfile = chat.chatProfiles.find((profile) => profile.id === chat.activeChatProfileId);
  const modelLabel = activeProfile?.model_id || activeProfile?.display_name || "未配置模型";
  const go = (id: SectionId) => { setActiveSection(id); setSearchOpen(false); setModelOpen(false); };
  useEffect(() => {
    const onKnowledgeContext = (event: Event) => {
      const content = (event as CustomEvent<{ content?: unknown }>).detail?.content;
      if (typeof content !== "string" || !content.trim()) return;
      chat.onNewConversation(content);
      setActiveSection("chat");
      setSearchOpen(false);
      setModelOpen(false);
    };
    window.addEventListener("suna:knowledge-context", onKnowledgeContext);
    return () => window.removeEventListener("suna:knowledge-context", onKnowledgeContext);
  }, [chat.onNewConversation, chat.onDraftChange]);
  const openConversation = (id: string) => { chat.onSelectConversation(id); go("chat"); };
  const handleSearchSelect = (item: CommandMenuItem) => {
    const conversation = item.payload as { conversation_id?: string } | undefined;
    if (conversation?.conversation_id) { openConversation(conversation.conversation_id); return; }
    go(item.section ?? "chat");
  };
  const content = activeSection === "chat" ? <ChatPage onOpenSection={go} /> : activeSection === "knowledge" ? <KnowledgeCenterPage /> : activeSection === "settings" ? <SettingsPage onOpenDiagnostics={() => go("diagnostics")} /> : activeSection === "diagnostics" ? <DiagnosticsPage /> : <ModuleView section={activeSection} onOpenSettings={() => go("settings")} />;
  const recent = chat.conversations.filter((conversation) => !conversation.archived).slice(0, 8);
  const matchingSections = navigationSections.filter((item) => !searchQuery.trim() || `${item.label}${item.description}`.toLocaleLowerCase().includes(searchQuery.trim().toLocaleLowerCase()));
  // 第 72 章：命令面板分组 = 页面与设置 / 对话 / 知识库 / Agent / Skill / MCP / 模型 / 文献。
  const searchGroups: CommandMenuGroup[] = (() => {
    if (!searchQuery.trim()) return [];
    const groups: CommandMenuGroup[] = [];
    if (matchingSections.length > 0) {
      groups.push({
        id: "sections",
        label: "页面与设置",
        items: matchingSections.slice(0, 5).map((item) => ({
          id: `section:${item.id}`,
          title: item.label,
          subtitle: item.description,
          section: item.id,
          icon: item.icon,
        })),
      });
    }
    if (searchResults.length > 0) {
      groups.push({
        id: "conversations",
        label: "对话",
        items: searchResults.slice(0, 6).map((hit) => ({
          id: `conversation:${hit.conversation_id}:${hit.message_id}`,
          title: hit.conversation_title,
          subtitle: hit.snippet,
          icon: MessageSquareText,
          payload: hit,
        })),
      });
    }
    groups.push(...filterGlobalEntities(entities, searchQuery));
    if (literatureItems.length > 0) {
      groups.push({ id: "literature", label: "文献", items: literatureItems });
    }
    return groups;
  })();
  return <div className={`suna-new-app ${sidebarCollapsed ? "is-sidebar-collapsed" : ""}`}><aside className="suna-new-sidebar"><div className="suna-new-brand"><SunaMark /><div><strong>Suna</strong><span>Steel Research Agent</span></div></div><button className="suna-new-create" onClick={() => { void chat.onNewConversation(); go("chat"); }}><Plus size={17} />新对话 <kbd>{shortcuts.newConversation.replace("+", " + ")}</kbd></button><div className="suna-new-nav-scroll"><p className="suna-new-nav-label">研究工作区</p>{primaryNavigationSections.map((item) => <NavButton key={item.id} item={item} active={activeSection} onClick={go} />)}<p className="suna-new-nav-label suna-new-nav-label-more">更多</p>{utilityNavigationSections.filter((item) => !["settings", "diagnostics"].includes(item.id)).map((item) => <NavButton key={item.id} item={item} active={activeSection} onClick={go} />)}<p className="suna-new-nav-label suna-new-nav-label-more">最近对话</p><div className="suna-new-history-list">{chat.loading ? <span className="suna-new-history-empty">正在加载...</span> : recent.length === 0 ? <span className="suna-new-history-empty">还没有对话</span> : recent.map((conversation) => <button key={conversation.id} className={`suna-new-history-item ${conversation.id === chat.selectedId ? "is-active" : ""}`} onClick={() => openConversation(conversation.id)} title={conversation.title}><span>{conversation.pinned ? "•" : ""}</span>{conversation.title.trim() || "新建对话"}</button>)}</div></div><div className="suna-new-sidebar-bottom"><button className="suna-new-bottom-link" onClick={() => setSearchOpen(true)}><Search size={16} />全局搜索 <kbd>{shortcuts.openCommand.replace("+", " + ")}</kbd></button><button className="suna-new-bottom-link" onClick={() => go("settings")}><Settings2 size={16} />设置中心</button><button className="suna-new-user" onClick={() => go("settings")}><span>TS</span><div><strong>{accountName}</strong><small>个人账户</small></div><ChevronRight size={15} /></button></div></aside><main className="suna-new-main"><header className="suna-new-header"><div className="suna-new-breadcrumb"><House size={16} /><strong>{activeSection === "chat" ? "新对话" : active.label}</strong>{activeSection === "chat" && <button className="suna-header-new-button" onClick={() => { void chat.onNewConversation(); }} aria-label="新建对话"><Plus size={18} /></button>}</div><div className="suna-new-header-actions"><div className="suna-model-menu-anchor"><button className="suna-header-model" onClick={() => setModelOpen((value) => !value)} aria-expanded={modelOpen}>{modelLabel} <ChevronRight size={13} className={modelOpen ? "is-open" : undefined} /></button>{modelOpen && <div className="suna-header-model-menu" role="menu">{chat.chatProfiles.length === 0 ? <button onClick={() => go("settings")}>前往设置配置模型</button> : chat.chatProfiles.map((profile) => <button key={profile.id} className={profile.id === chat.activeChatProfileId ? "is-active" : ""} onClick={() => { void chat.onSelectChatProfile(profile.id); setModelOpen(false); }}>{profile.model_id || profile.display_name}</button>)}</div>}</div><button className="suna-header-icon" onClick={() => setThemePreference(resolvedTheme === "dark" ? "light" : "dark")} aria-label="切换主题" aria-pressed={resolvedTheme === "dark"}><Sun size={17} /></button><button className="suna-header-icon" onClick={() => go("settings")} aria-label="设置"><Settings2 size={17} /></button><button className="suna-header-icon" onClick={() => setSidebarCollapsed((collapsed) => !collapsed)} aria-label="布局" aria-pressed={sidebarCollapsed}><LayoutGrid size={17} /></button></div></header><div className="suna-new-content">{content}</div></main><CommandMenu open={searchOpen} onClose={() => setSearchOpen(false)} query={searchQuery} onQueryChange={setSearchQuery} inputRef={searchRef} groups={searchGroups} onSelect={handleSearchSelect} /></div>;
}
