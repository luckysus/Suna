import { Activity, AlertCircle, Check, MoreHorizontal, RefreshCw } from "lucide-react";
import LanguageSelect from "../../components/common/LanguageSelect";
import type { MessageKey } from "../../i18n/locale";
import { Button } from "../../components/ui/button";

interface Props {
  t: (key: MessageKey, values?: Record<string, string | number>) => string;
  loading: boolean;
  error: string | null;
  notice: string | null;
  load: () => void;
  exportSettings: () => void;
  importSettings: (file: File) => void;
  fileInputKey: number;
  resetPreferences: () => void;
  onOpenDiagnostics?: () => void;
  showUtilities?: boolean;
}

export default function SettingsPageChrome({
  t,
  loading,
  error,
  notice,
  load,
  exportSettings,
  importSettings,
  fileInputKey,
  resetPreferences,
  onOpenDiagnostics,
  showUtilities = true,
}: Props) {
  return <>
    <header className="suna-settings-header">
      <div>
        <h1 id="settings-heading">{t("settingsTitle")}</h1>
        <p>{t("settingsCenterLede")}</p>
      </div>
      {showUtilities && <div className="suna-settings-header-actions">
        <LanguageSelect />
        {onOpenDiagnostics && <Button size="icon" type="button" onClick={onOpenDiagnostics} aria-label={t("settingsDiagnostics")} title={t("settingsDiagnostics")}><Activity size={17} aria-hidden="true" /></Button>}
        <Button size="icon" type="button" onClick={load} disabled={loading} aria-label={t("settingsRefresh")} title={t("settingsRefresh")}><RefreshCw size={17} aria-hidden="true" className={loading ? "suna-spin" : undefined} /></Button>
        <details className="suna-settings-more">
          <summary aria-label={t("settingsMore")} title={t("settingsMore")}><MoreHorizontal size={18} aria-hidden="true" /></summary>
          <div className="suna-settings-more-menu">
            <button type="button" onClick={exportSettings}>{t("settingsExport")}</button>
            <label>{t("settingsImport")}<input key={fileInputKey} type="file" accept="application/json" hidden onChange={(event) => { const file = event.target.files?.[0]; if (file) importSettings(file); }} /></label>
            <button type="button" onClick={resetPreferences}>{t("settingsReset")}</button>
          </div>
        </details>
      </div>}
    </header>
    {error && <div className="suna-settings-alert" role="alert"><AlertCircle size={17} aria-hidden="true" /><span>{error}</span></div>}
    {notice && <div className="suna-settings-notice" role="status"><Check size={17} aria-hidden="true" /><span>{notice}</span></div>}
  </>;
}