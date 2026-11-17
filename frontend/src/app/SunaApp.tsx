import { useEffect, useMemo, useRef, useState } from "react";
import { Archive, ChevronDown, ChevronRight, LayoutGrid, MessageSquareText, Moon, PanelLeftClose, PanelLeftOpen, Pencil, Pin, PinOff, Plus, Search, Settings2, Sun, Trash2 } from "lucide-react";
import { Avatar, AvatarFallback } from "../components/ui/avatar";
import { desktop, isDesktopRuntime } from "../bridge/desktop";
import ChatPage from "../features/chat/ChatPage";
import { ChatControllerProvider } from "../features/chat/chatController";
import { useChatControllerContext } from "../features/chat/chatController";
import ResearchProjectContext from "../features/research/ResearchProjectContext";
import { getLocalizedSection, navigationSections, primaryNavigationSections, sidebarManageSections, utilityNavigationSections, type NavigationSection, type SectionId } from "./navigation";
import { isRootPath, useSectionRoute } from "./sectionRoute";
import SunaTitleBar from "./SunaTitleBar";
import { defaultShortcuts, normalizeShortcuts, parseShortcut, type ShortcutSettings } from "./shortcuts";
import { filterGlobalEntities, loadGlobalEntities, searchLiteratureItems, type GlobalEntity } from "./globalSearch";
import { CommandMenu, type CommandMenuGroup, type CommandMenuItem } from "../components/ui/command-menu";
import { ToastProvider, useToast } from "../components/ui/toast";
import { LocaleProvider, useLocale } from "../i18n/locale";
import { ThemeProvider, useTheme } from "../theme/theme";
import { AppearanceProvider, useAppearanceSettings } from "../settings/appearance";
import SunaLogo from "../components/SunaLogo";
import ArchivedConversationsDialog from "../features/chat/ArchivedConversationsDialog";
import { deferredPage } from "./deferredPage";
import { dismissBootScreen, markBoot } from "../startupSplash";
import "./suna-shell.css";

const DiagnosticsPage = deferredPage(() => import("../features/diagnostics/DiagnosticsPage"));
const KnowledgeCenterPage = deferredPage(() => import("../features/knowledge/KnowledgeCenterPage"));
const ResearchProjectsPage = deferredPage(() => import("../features/research/ResearchProjectsPage"));
const ResearchModulePage = deferredPage(() => import("../features/research/ResearchModulePage"));
const LiteratureResearchPage = deferredPage(() => import("../features/literature/LiteratureResearchPage"));
const PerformancePredictionPage = deferredPage(() => import("../features/prediction/PerformancePredictionPage"));
const ProcessOptimizationPage = deferredPage(() => import("../features/optimization/ProcessOptimizationPage"));
const McpManagementPage = deferredPage(() => import("../features/mcp/McpManagementPage"));
const ExperimentAssistantPage = deferredPage(() => import("../features/experiment/ExperimentAssistantPage"));
const ResearchReportPage = deferredPage(() => import("../features/reports/ResearchReportPage"));
const DataLabPage = deferredPage(() => import("../features/data/DataLabPage"));
const CapabilityManagementPage = deferredPage(() => import("../features/management/CapabilityManagementPage"));
const SettingsPage = deferredPage(() => import("../features/settings/SettingsPage"));

/** 设置、诊断与能力管理不依赖当前课题，避免顶栏干扰。 */
const SECTIONS_WITHOUT_PROJECT_CONTEXT = new Set<SectionId>([
  "settings",
  "diagnostics",
  "agents",
  "tools",
  "mcp",
  "skills",
  "models",
  "account",
  "about",
]);

function showsResearchProjectContext(section: SectionId): boolean {
  return !SECTIONS_WITHOUT_PROJECT_CONTEXT.has(section);
}

export default function SunaApp() { return <ToastProvider><LocaleProvider><ThemeProvider><AppearanceProvider><ChatControllerProvider><SunaAppShell /></ChatControllerProvider></AppearanceProvider></ThemeProvider></LocaleProvider></ToastProvider>; }
function SunaMark() { return <SunaLogo size={37} title="Suna" aria-hidden="true" />; }
function ModuleView({ section, onOpenSettings }: { section: SectionId; onOpenSettings: () => void }) { return section === "projects" ? <ResearchProjectsPage /> : section === "literature" ? <LiteratureResearchPage /> : section === "prediction" ? <PerformancePredictionPage /> : section === "optimization" ? <ProcessOptimizationPage /> : section === "mcp" ? <McpManagementPage /> : section === "experiment" ? <ExperimentAssistantPage /> : section === "reports" ? <ResearchReportPage /> : section === "data" ? <DataLabPage /> : section === "agents" || section === "tools" || section === "skills" ? <CapabilityManagementPage section={section} /> : section === "models" ? <SettingsPage onOpenDiagnostics={onOpenSettings} initialTab="providers" /> : section === "account" ? <SettingsPage onOpenDiagnostics={onOpenSettings} initialTab="account" /> : section === "about" ? <SettingsPage onOpenDiagnostics={onOpenSettings} initialTab="about" /> : <ResearchModulePage section={section} />; }
function NavButton({ item, active, onClick, onPrefetch }: { item: NavigationSection; active: SectionId; onClick: (id: SectionId) => void; onPrefetch?: (id: SectionId) => void }) { const Icon = item.icon; return <button className={`suna-new-nav-item ${active === item.id ? "is-active" : ""}`} title={item.label} aria-label={item.label} onClick={() => onClick(item.id)} onMouseEnter={() => onPrefetch?.(item.id)} onFocus={() => onPrefetch?.(item.id)} onPointerDown={() => onPrefetch?.(item.id)}><Icon size={17} /><span>{item.label}</span></button>; }
function SunaAppShell() {
  const { t } = useLocale();
  const chat = useChatControllerContext();
  const toast = useToast();
  const { preferences, loaded: appearanceLoaded, updatePreferences } = useAppearanceSettings();
  const { resolvedTheme, setPreference: setThemePreference } = useTheme();
  const startupApplied = useRef(false);
  const bootDismissScheduled = useRef(false);
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
  const [historyExpanded, setHistoryExpanded] = useState(false);
  const [archivedOpen, setArchivedOpen] = useState(false);
  useEffect(() => {
    if (!appearanceLoaded || startupApplied.current) return;
    startupApplied.current = true;
    if (preferences.startupPage !== "chat" && isRootPath(window.location.pathname)) setActiveSection(preferences.startupPage);
  }, [appearanceLoaded, preferences.startupPage]);
  useEffect(() => {
    if (!appearanceLoaded || bootDismissScheduled.current) return;
    bootDismissScheduled.current = true;
    markBoot("appearance-loaded");
    // 外观偏好重排与启动页重定向提交、字体就绪后再淡出，让重排全部发生在启动画面底下。
    const fontsReady = document.fonts?.ready ?? Promise.resolve();
    void fontsReady.catch(() => undefined).then(() => {
      markBoot("fonts-ready");
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          markBoot("dismiss");
          dismissBootScreen();
        });
      });
    });
  }, [appearanceLoaded]);
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
      if (matches(event, shortcuts.recoverTask)) { event.preventDefault(); chat.onResume(); return; }
      if (event.key === "Escape") { setSearchOpen(false); setModelOpen(false); }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [chat.onCancel, chat.onNewConversation, chat.onResume, shortcuts]);
  useEffect(()=>{const restore=()=>{if(window.location.pathname==='/chat'){const id=new URLSearchParams(window.location.search).get('conversation');if(id)void chat.onSelectConversation(id);}};restore();window.addEventListener('popstate',restore);return()=>window.removeEventListener('popstate',restore);},[activeSection,chat.onSelectConversation]);
  const localizedPrimarySections = useMemo(
    () => primaryNavigationSections.map((item) => getLocalizedSection(item, t)),
    [t],
  );
  const localizedSidebarManageSections = useMemo(
    () => sidebarManageSections.map((item) => getLocalizedSection(item, t)),
    [t],
  );
  const localizedAllSections = useMemo(
    () => navigationSections.map((item) => getLocalizedSection(item, t)),
    [t],
  );
  const activeProfile = chat.chatProfiles.find((profile) => profile.id === chat.activeChatProfileId);
  const go = (id: SectionId) => { setActiveSection(id); setSearchOpen(false); setModelOpen(false); };
  const prefetchSection = (id: SectionId) => {
    switch (id) {
      case "diagnostics": void DiagnosticsPage.preload(); break;
      case "knowledge": void KnowledgeCenterPage.preload(); break;
      case "projects": void ResearchProjectsPage.preload(); break;
      case "literature": void LiteratureResearchPage.preload(); break;
      case "prediction": void PerformancePredictionPage.preload(); break;
      case "optimization": void ProcessOptimizationPage.preload(); break;
      case "mcp": void McpManagementPage.preload(); break;
      case "experiment": void ExperimentAssistantPage.preload(); break;
      case "reports": void ResearchReportPage.preload(); break;
      case "data": void DataLabPage.preload(); break;
      case "agents":
      case "tools":
      case "skills":
        void CapabilityManagementPage.preload();
        break;
      case "settings":
      case "models":
      case "account":
      case "about":
        void SettingsPage.preload();
        break;
    }
  };
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
  useEffect(() => {
    const onOpenModelLibrary = (event: Event) => {
      const detail = (event as CustomEvent<{ modelId?: string }>).detail ?? {};
      go("models");
      if (detail.modelId) {
        window.setTimeout(() => {
          window.dispatchEvent(new CustomEvent("suna:model-library-focus", { detail: { modelId: detail.modelId } }));
        }, 0);
      }
    };
    const onNavigateSection = (event: Event) => {
      const section = (event as CustomEvent<{ section?: string }>).detail?.section;
      if (section) go(section as SectionId);
    };
    window.addEventListener("suna:open-model-library", onOpenModelLibrary);
    window.addEventListener("suna:navigate-section", onNavigateSection);
    const params = new URLSearchParams(window.location.search);
    const deepLinkModelId = params.get("model_id")?.trim();
    if (deepLinkModelId) {
      go("models");
      window.setTimeout(() => {
        window.dispatchEvent(
          new CustomEvent("suna:model-library-focus", { detail: { modelId: deepLinkModelId } }),
        );
      }, 0);
    }
    return () => {
      window.removeEventListener("suna:open-model-library", onOpenModelLibrary);
      window.removeEventListener("suna:navigate-section", onNavigateSection);
    };
  }, [go]);
  const openConversation = (id: string) => { chat.onSelectConversation(id); go("chat"); };
  const handleSearchSelect = (item: CommandMenuItem) => {
    const conversation = item.payload as { conversation_id?: string } | undefined;
    if (conversation?.conversation_id) { openConversation(conversation.conversation_id); return; }
    go(item.section ?? "chat");
  };
  const content = activeSection === "chat" ? <ChatPage onOpenSection={go} /> : activeSection === "knowledge" ? <KnowledgeCenterPage /> : activeSection === "settings" ? <SettingsPage onOpenDiagnostics={() => go("diagnostics")} /> : activeSection === "diagnostics" ? <DiagnosticsPage /> : <ModuleView section={activeSection} onOpenSettings={() => go("settings")} />;
  const recent = chat.conversations.filter((conversation) => !conversation.archived);
  const matchingSections = localizedAllSections.filter((item) => !searchQuery.trim() || `${item.label}${item.description}`.toLocaleLowerCase().includes(searchQuery.trim().toLocaleLowerCase()));
  // 命令面板分组 = 页面与设置 / 对话 / 知识库 / Agent / Skill / MCP / 模型 / 文献。
  const searchGroups: CommandMenuGroup[] = (() => {
    if (!searchQuery.trim()) return [];
    const groups: CommandMenuGroup[] = [];
    if (matchingSections.length > 0) {
      groups.push({
        id: "sections",
        label: t("searchGroupSections"),
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
        label: t("chatTitle"),
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
      groups.push({ id: "literature", label: t("literatureReference"), items: literatureItems });
    }
    return groups;
  })();
  const desktopShell = isDesktopRuntime();
  return (
    <div className={`suna-new-app ${sidebarCollapsed ? "is-sidebar-collapsed" : ""}${desktopShell ? " is-desktop-shell" : ""}`}>
      {desktopShell && <SunaTitleBar />}
      <div className="suna-new-body">
      <aside className={`suna-new-sidebar ${sidebarCollapsed ? "is-collapsed" : ""}`}>
        <div className="suna-new-brand">
          <div className="suna-new-brand-content">
            <SunaMark />
            <div className="suna-new-brand-text">
              <strong>Suna</strong>
              <span>Steel Research Agent</span>
            </div>
          </div>
          <button
            type="button"
            className="suna-sidebar-toggle-btn"
            onClick={() => setSidebarCollapsed((c) => !c)}
            title={sidebarCollapsed ? "展开侧边栏" : "折叠侧边栏"}
            aria-label={sidebarCollapsed ? "展开侧边栏" : "折叠侧边栏"}
          >
            {sidebarCollapsed ? <PanelLeftOpen size={16} /> : <PanelLeftClose size={16} />}
          </button>
        </div>
        <button className="suna-new-create" onClick={() => { void chat.onNewConversation(); go("chat"); }} title={t("newConversation")} aria-label={t("newConversation")}>
          <div className="suna-new-create-left">
            <Plus size={16} />
            <span>{t("newConversation")}</span>
          </div>
          <kbd>{shortcuts.newConversation.replace("+", " + ")}</kbd>
        </button>
        <div className="suna-new-nav-scroll">
          <p className="suna-new-nav-label">{t("navGroupResearch")}</p>
          {localizedPrimarySections.map((item) => <NavButton key={item.id} item={item} active={activeSection} onClick={go} onPrefetch={prefetchSection} />)}
          <p className="suna-new-nav-label suna-new-nav-label-more">{t("navGroupMore")}</p>
          {localizedSidebarManageSections.map((item) => <NavButton key={item.id} item={item} active={activeSection} onClick={go} onPrefetch={prefetchSection} />)}
          <div className="suna-new-history-header">
            <p className="suna-new-nav-label suna-new-nav-label-more">{t("navRecentConversations")}</p>
            <button
              type="button"
              className="suna-new-history-archived-toggle"
              title={t("navArchived")}
              aria-label={t("navArchived")}
              onClick={() => setArchivedOpen(true)}
            >
              <Archive size={12} />
              <span>{t("navArchived")}</span>
            </button>
          </div>
          <div className="suna-new-history-list">
            {chat.loading ? (
              <span className="suna-new-history-empty">{t("loading")}...</span>
            ) : recent.length === 0 ? (
              <span className="suna-new-history-empty">{t("navNoConversations")}</span>
            ) : (
              (historyExpanded ? recent : recent.slice(0, 5)).map((conversation) => (
                <div className="suna-new-history-item-wrap" key={conversation.id}>
                  <button
                    type="button"
                    className={`suna-new-history-item ${conversation.id === chat.selectedId ? "is-active" : ""}`}
                    onClick={() => openConversation(conversation.id)}
                    title={conversation.title}
                  >
                    {conversation.pinned && <Pin size={11} className="suna-new-history-pin-icon" />}
                    <span className="suna-new-history-title">{conversation.title.trim() || t("newConversation")}</span>
                  </button>
                  <div className="suna-new-history-actions">
                    <button
                      type="button"
                      className={`suna-new-history-action ${conversation.pinned ? "is-pinned" : ""}`}
                      title={conversation.pinned ? t("navUnpinConversation") : t("navPinConversation")}
                      aria-label={conversation.pinned ? t("navUnpinConversation") : t("navPinConversation")}
                      onClick={(e) => {
                        e.stopPropagation();
                        void chat.onToggleConversationPinned(conversation);
                      }}
                    >
                      {conversation.pinned ? <PinOff size={12} /> : <Pin size={12} />}
                    </button>
                    <button
                      type="button"
                      className="suna-new-history-action"
                      title={t("navRenameConversation")}
                      aria-label={t("navRenameConversation")}
                      onClick={(e) => {
                        e.stopPropagation();
                        const nextTitle = window.prompt(t("navRenamePrompt"), conversation.title);
                        if (nextTitle && nextTitle.trim()) {
                          void chat.onRenameConversation(conversation.id, nextTitle.trim());
                        }
                      }}
                    >
                      <Pencil size={12} />
                    </button>
                    <button
                      type="button"
                      className="suna-new-history-action"
                      title={t("navArchiveConversation")}
                      aria-label={t("navArchiveConversation")}
                      onClick={(e) => {
                        e.stopPropagation();
                        void chat.onArchiveConversation(conversation.id);
                      }}
                    >
                      <Archive size={12} />
                    </button>
                    <button
                      type="button"
                      className="suna-new-history-action is-danger"
                      title={t("navDeleteConversation")}
                      aria-label={t("navDeleteConversation")}
                      onClick={(e) => {
                        e.stopPropagation();
                        void chat.onDeleteConversation(conversation.id);
                      }}
                    >
                      <Trash2 size={12} />
                    </button>
                  </div>
                </div>
              ))
            )}
            {recent.length > 5 && (
              <button
                type="button"
                className="suna-new-history-more-toggle"
                onClick={() => setHistoryExpanded((exp) => !exp)}
                aria-expanded={historyExpanded}
              >
                <ChevronDown size={13} className={historyExpanded ? "rotate-180" : ""} />
                <span>{historyExpanded ? "收起较早会话" : `显示更多会话 (${recent.length - 5})`}</span>
              </button>
            )}
          </div>
        </div>
        <div className="suna-new-sidebar-bottom">
          <button className="suna-new-bottom-link" onClick={() => setSearchOpen(true)}>
            <Search size={16} />{t("navGlobalSearch")} <kbd>{shortcuts.openCommand.replace("+", " + ")}</kbd>
          </button>
          <button
            className="suna-new-bottom-link"
            onClick={() => go("settings")}
            onMouseEnter={() => prefetchSection("settings")}
            onFocus={() => prefetchSection("settings")}
            onPointerDown={() => prefetchSection("settings")}
          >
            <Settings2 size={16} />{t("navSettings")}
          </button>
          <button
            className="suna-new-user"
            onClick={() => go("settings")}
            onMouseEnter={() => prefetchSection("settings")}
            onFocus={() => prefetchSection("settings")}
            onPointerDown={() => prefetchSection("settings")}
          >
            <Avatar className="h-7 w-7 text-xs">
              <AvatarFallback>{accountName.slice(0, 2).toUpperCase()}</AvatarFallback>
            </Avatar>
            <div>
              <strong>{accountName}</strong>
              <small>{t("navAccount")}</small>
            </div>
            <ChevronRight size={15} />
          </button>
        </div>
      </aside>
      <main className="suna-new-main">
        {/* 各业务页自带标题区；壳层面包屑会与页面 h1 重复，统一不再渲染。 */}
        {showsResearchProjectContext(activeSection) && <ResearchProjectContext />}
        <div className="suna-new-content">{content}</div>
      </main>
      </div>
      <CommandMenu
        open={searchOpen}
        onClose={() => setSearchOpen(false)}
        query={searchQuery}
        onQueryChange={setSearchQuery}
        inputRef={searchRef}
        groups={searchGroups}
        onSelect={handleSearchSelect}
      />
      <ArchivedConversationsDialog
        open={archivedOpen}
        onOpenChange={setArchivedOpen}
        onSelectConversation={openConversation}
      />
    </div>
  );
}
