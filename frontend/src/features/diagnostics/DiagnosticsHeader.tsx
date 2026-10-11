import { Download, RefreshCw, Upload } from "lucide-react";
import { useLocale } from "../../i18n/locale";
import { Button } from "../../components/ui/button";

interface Props {
  loading: boolean;
  busy: boolean;
  onRefresh: () => void;
  onExport: () => void;
  onCreateBackup: () => void;
  onRestoreBackup: () => void;
}

export default function DiagnosticsHeader({
  loading,
  busy,
  onRefresh,
  onExport,
  onCreateBackup,
  onRestoreBackup,
}: Props) {
  const { t } = useLocale();
  return (
    <header className="suna-diagnostics-header">
      <div>
        <h1 id="diagnostics-heading">{t("diagnosticsTitle")}</h1>
      </div>
      <div className="suna-diagnostics-actions">
        <Button size="icon" type="button" onClick={onRefresh} disabled={loading} aria-label={t("diagnosticsRefresh")} title={t("diagnosticsRefresh")}>
          <RefreshCw size={18} aria-hidden="true" />
        </Button>
        <Button variant="actionSecondary" type="button" onClick={onExport} disabled={loading || busy}>
          <Download size={16} aria-hidden="true" />{t("diagnosticsExport")}
        </Button>
        <Button variant="actionSecondary" type="button" onClick={onCreateBackup} disabled={loading || busy}>
          <Download size={16} aria-hidden="true" />{t("diagnosticsBackupExport")}
        </Button>
        <Button variant="actionSecondary" type="button" onClick={onRestoreBackup} disabled={loading || busy}>
          <Upload size={16} aria-hidden="true" />{t("diagnosticsBackupRestore")}
        </Button>
      </div>
    </header>
  );
}
