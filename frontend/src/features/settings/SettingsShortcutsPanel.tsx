import { useEffect, useRef, useState } from "react";
import { Check, Keyboard, RotateCcw, Save } from "lucide-react";
import { getSettingValue, setSettingValue } from "./settingsModel";
import { settingsErrorMessage } from "./settingsError";
import { defaultShortcuts, normalizeShortcuts, shortcutChoices, shortcutRows, type ShortcutKey as ShortcutKeyType } from "../../app/shortcuts";
import { useDebouncedCallback } from "../../hooks/useDebouncedCallback";
import { useLocale } from "../../i18n/locale";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../../components/ui/select";
import { Button } from "../../components/ui/button";

type ShortcutKey = ShortcutKeyType;
type ShortcutSettings = Record<ShortcutKey, string>;
const defaults: ShortcutSettings = defaultShortcuts;
const rows = shortcutRows;
const choices = shortcutChoices;
const normalize = (raw: string | null): ShortcutSettings => normalizeShortcuts(raw);
export default function SettingsShortcutsPanel() {
  const { t } = useLocale(); const [value, setValue] = useState<ShortcutSettings>(defaults); const [loaded, setLoaded] = useState(false); const [loadNonce, setLoadNonce] = useState(0); const [loadError, setLoadError] = useState(false); const [saved, setSaved] = useState(false); const [saving, setSaving] = useState(false); const [error, setError] = useState<string | null>(null); const pending = useRef(defaults); const request = useRef(0);
  useEffect(() => {
    let active = true;
    const load = async () => {
      setLoaded(false);
      setLoadError(false);
      setError(null);
      try {
        const raw = await getSettingValue("ui.shortcuts");
        if (!active) return;
        const next = normalize(raw); pending.current = next; setValue(next); setSaved(false); setLoadError(false);
      } catch (cause) {
        if (active) { setLoadError(true); setError(settingsErrorMessage(cause, "无法读取快捷键，请重试")); }
      } finally {
        if (active) setLoaded(true);
      }
    };
    void load();
    const changed = (event: Event) => {
      const detail = (event as CustomEvent<{ key?: string; value?: string }>).detail;
      if (detail?.key !== "ui.shortcuts" || !detail.value) return;
      const next = normalize(detail.value); pending.current = next; setValue(next); setSaved(true); setLoadError(false); setError(null);
    };
    const reset = () => { pending.current = defaults; setValue(defaults); setSaved(true); setLoadError(false); setError(null); };
    window.addEventListener("suna:setting-changed", changed);
    window.addEventListener("suna:settings-reset", reset);
    return () => { active = false; window.removeEventListener("suna:setting-changed", changed); window.removeEventListener("suna:settings-reset", reset); };
  }, [loadNonce]);
  const duplicateFor = (next: ShortcutSettings) => Object.values(next).filter((shortcut) => shortcut !== "未设置").some((shortcut, index, values) => values.indexOf(shortcut) !== index);
  const persist = async (next: ShortcutSettings) => {
    if (duplicateFor(next)) { setError("快捷键不能重复，请修改冲突的操作"); setSaved(false); return; }
    const currentRequest = ++request.current;
    setSaving(true); setError(null); setSaved(false);
    try { await setSettingValue("ui.shortcuts", JSON.stringify({ version: 1, ...next })); if (currentRequest === request.current) { setLoadError(false); setSaved(true); } }
    catch (cause) { if (currentRequest === request.current) { setLoadError(false); setError(settingsErrorMessage(cause, "快捷键保存失败，请重试")); } }
    finally { if (currentRequest === request.current) setSaving(false); }
  };
  const { schedule: schedulePersist, cancel: cancelPersist } = useDebouncedCallback((next: ShortcutSettings) => { void persist(next); }, 650);
  const update = (key: ShortcutKey, nextShortcut: string) => { const next = { ...pending.current, [key]: nextShortcut }; pending.current = next; setValue(next); setSaved(false); setError(null); schedulePersist(next); };
  const duplicate = duplicateFor(value);
  const save = async () => { cancelPersist(); await persist(pending.current); };
  const reset = () => { cancelPersist(); pending.current = defaults; setValue(defaults); setSaved(false); setError(null); schedulePersist(defaults); };
  const retryLoad = () => { setLoadNonce((nonce) => nonce + 1); };
  return <section className="suna-settings-form-panel" aria-labelledby="settings-shortcuts-heading" aria-busy={!loaded || saving}><header className="suna-settings-form-heading"><div><span className="suna-settings-kicker">KEYBOARD</span><h2 id="settings-shortcuts-heading">{t("settingsCategoryShortcuts")}</h2><p>{t("settingsShortcutsCopy")}</p></div><Keyboard size={22} aria-hidden="true" /></header>{!loaded && <p className="suna-settings-loading" role="status">正在加载快捷键...</p>}{error && !duplicate && <p className="suna-settings-inline-error" role="alert">{error}{loadError && <button type="button" className="suna-settings-inline-retry" onClick={retryLoad}>重试</button>}</p>}{duplicate && <p className="suna-settings-inline-error" role="alert">快捷键不能重复，请修改冲突的操作。</p>}<div className="suna-shortcut-list">{rows.map((row) => <label className="suna-shortcut-row" key={row.key}><span><strong>{row.label}</strong><small>{row.copy}</small></span><Select value={value[row.key]} aria-label={row.label} onValueChange={(value) => update(row.key, value)}>
  <SelectTrigger aria-label={row.label}><SelectValue /></SelectTrigger>
  <SelectContent>{choices.map((choice) => <SelectItem key={choice} value={choice}>{choice}</SelectItem>)}
  </SelectContent>
</Select></label>)}</div><footer className="suna-settings-form-actions"><Button variant="secondary" type="button" onClick={reset}><RotateCcw size={15} />恢复默认快捷键</Button><Button variant="primary" type="button" onClick={() => void save()} disabled={duplicate || saving}><Save size={15} />{saving ? "保存中" : "立即保存"}</Button><span className={`suna-settings-inline-saved is-${saving ? "saving" : saved ? "saved" : "idle"}`} role="status">{saving && <span>保存中</span>}{saved && <><Check size={14} />{t("settingsSaved")}</>}{!saving && !saved && !duplicate && <span>自动保存</span>}</span></footer></section>;
}


