import { useEffect, useMemo, useState } from "react";
import { Archive, Clock, ExternalLink, RotateCcw, Search, Trash2 } from "lucide-react";
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "../../components/ui/dialog";
import { Input } from "../../components/ui/input";
import { desktop, type Conversation } from "../../bridge/desktop";

interface ArchivedConversationsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onSelectConversation?: (id: string) => void;
}

export default function ArchivedConversationsDialog({
  open,
  onOpenChange,
  onSelectConversation,
}: ArchivedConversationsDialogProps) {
  const [archived, setArchived] = useState<Conversation[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(false);

  const fetchArchived = async () => {
    setLoading(true);
    try {
      const list = await desktop.listArchivedConversations();
      setArchived(list);
    } catch {
      setArchived([]);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (open) {
      setSearch("");
      void fetchArchived();
    }
  }, [open]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    return q ? archived.filter((item) => item.title.toLowerCase().includes(q)) : archived;
  }, [archived, search]);

  const handleRestore = async (id: string) => {
    try {
      await desktop.restoreConversation(id);
      window.dispatchEvent(new CustomEvent("suna:conversation-restored", { detail: { id } }));
      void fetchArchived();
    } catch {
      // Keep UI stable if restore fails
    }
  };

  const handleRestoreAndOpen = async (id: string) => {
    await handleRestore(id);
    onOpenChange(false);
    onSelectConversation?.(id);
  };

  const handleDelete = async (id: string) => {
    if (!window.confirm("确定永久删除此归档对话吗？删除后将无法恢复。")) return;
    try {
      await desktop.deleteConversationLocal(id);
      void fetchArchived();
    } catch {
      // Keep UI stable if delete fails
    }
  };

  const formatDate = (isoString?: string) => {
    if (!isoString) return "";
    try {
      const d = new Date(isoString);
      return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
    } catch {
      return isoString;
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="suna-archived-dialog max-w-xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2 text-base font-semibold">
            <Archive size={18} className="text-primary" />
            <span>已归档对话</span>
            <span className="ml-1 rounded-full bg-muted px-2 py-0.5 text-xs font-mono text-muted-foreground">
              {archived.length}
            </span>
          </DialogTitle>
          <DialogDescription className="text-xs text-muted-foreground">
            归档对话不会在侧边栏最近列表中展示，历史记录完整保留，可随时恢复至最近列表或永久删除。
          </DialogDescription>
        </DialogHeader>

        <div className="relative mt-2">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索归档对话标题..."
            className="h-8 pl-8 text-xs"
          />
        </div>

        <div className="mt-2 max-h-[360px] min-h-[160px] overflow-y-auto space-y-1.5 pr-1">
          {loading ? (
            <div className="flex h-36 items-center justify-center text-xs text-muted-foreground">正在加载归档对话...</div>
          ) : filtered.length === 0 ? (
            <div className="flex flex-col items-center justify-center py-12 text-center text-muted-foreground">
              <Archive size={28} className="opacity-30 mb-2" />
              <p className="text-xs font-medium">{search ? "没有匹配的归档对话" : "暂无已归档对话"}</p>
            </div>
          ) : (
            filtered.map((item) => (
              <div
                key={item.id}
                className="group flex items-center justify-between rounded-md border border-border/60 bg-card/60 p-2.5 transition-colors hover:bg-accent/40"
              >
                <div className="min-w-0 flex-1 pr-3">
                  <p className="truncate text-xs font-medium text-foreground">{item.title || "未命名对话"}</p>
                  <p className="flex items-center gap-1 text-[11px] font-mono text-muted-foreground mt-0.5">
                    <Clock size={11} />
                    <span>{formatDate(item.updated_at || item.created_at)}</span>
                  </p>
                </div>
                <div className="flex items-center gap-1 shrink-0">
                  <button
                    type="button"
                    title="恢复并打开对话"
                    onClick={() => void handleRestoreAndOpen(item.id)}
                    className="inline-flex h-7 items-center gap-1 rounded px-2 text-xs font-medium text-foreground transition-colors hover:bg-primary hover:text-primary-foreground"
                  >
                    <ExternalLink size={12} />
                    <span>恢复并打开</span>
                  </button>
                  <button
                    type="button"
                    title="仅恢复到最近对话"
                    onClick={() => void handleRestore(item.id)}
                    className="inline-flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                  >
                    <RotateCcw size={13} />
                  </button>
                  <button
                    type="button"
                    title="永久删除"
                    onClick={() => void handleDelete(item.id)}
                    className="inline-flex h-7 w-7 items-center justify-center rounded text-muted-foreground transition-colors hover:bg-destructive/15 hover:text-destructive"
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
