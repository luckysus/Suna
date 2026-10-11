import { useRef, useState, type DragEvent, type ReactNode } from "react";
import { Upload } from "lucide-react";
import { cn } from "../../lib/utils";
import { useNativeFileDrop } from "../../hooks/useNativeFileDrop";
import "./file-drop-zone.css";

/**
 * 第 68 章：通用 Drag & Drop。
 *
 * 原生 WebView 拖拽事件提供本地路径；扩展名不符或拿不到路径时反馈给调用方。
 */

export type FileDropZoneProps = {
  /** 接收通过校验的文件路径。 */
  onFiles: (paths: string[]) => void | Promise<void>;
  /** 允许的扩展名（小写，不含点）；留空表示不限制。 */
  extensions?: string[];
  disabled?: boolean;
  children: ReactNode;
  /** 拖拽悬停时显示的主文案。 */
  label?: string;
  /** 辅助说明（支持的格式等）。 */
  hint?: string;
  /** 被拒绝时回调（扩展名不符 / 无法读取路径）。 */
  onRejected?: (reason: string, paths: string[]) => void;
  className?: string;
};

function extensionOf(path: string): string {
  const name = path.split(/[\\/]/).pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

export function FileDropZone({
  onFiles,
  extensions = [],
  disabled = false,
  children,
  label = "松开即可导入",
  hint,
  onRejected,
  className,
}: FileDropZoneProps) {
  const [dragging, setDragging] = useState(false);
  const region = useRef<HTMLDivElement>(null);

  const accepts = (path: string) => extensions.length === 0 || extensions.includes(extensionOf(path));

  const acceptPaths = async (dropped: string[]) => {
    if (disabled) return;
    const readable = dropped.filter(Boolean);
    if (readable.length === 0) {
      onRejected?.("无法读取拖拽文件路径，请使用导入按钮选择文件", []);
      return;
    }
    const accepted = readable.filter(accepts);
    const rejected = readable.filter((path) => !accepts(path));
    if (rejected.length > 0) {
      onRejected?.(`已忽略 ${rejected.length} 个不支持的文件类型`, rejected);
    }
    if (accepted.length > 0) await onFiles(accepted);
  };
  useNativeFileDrop(region, { disabled, onFiles: acceptPaths, onHover: setDragging, onError: () => onRejected?.("拖拽导入失败，请使用导入按钮重试", []) });
  const handleDrop = async (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setDragging(false);
    await acceptPaths(Array.from(event.dataTransfer.files).map((file) => (file as File & { path?: string }).path ?? ""));
  };

  return (
    <div
      ref={region}
      className={cn("suna-drop-zone", dragging && "is-dragging", disabled && "is-disabled", className)}
      onDragOver={(event) => { if (!disabled) { event.preventDefault(); setDragging(true); } }}
      onDragEnter={(event) => { if (!disabled) { event.preventDefault(); setDragging(true); } }}
      onDragLeave={(event) => { if (event.currentTarget.contains(event.relatedTarget as Node)) return; setDragging(false); }}
      onDrop={(event) => void handleDrop(event)}
      data-drop-active={dragging ? "true" : undefined}
    >
      {children}
      {dragging && (
        <div className="suna-drop-zone-overlay" role="status">
          <Upload size={22} aria-hidden="true" />
          <strong>{label}</strong>
          {hint && <small>{hint}</small>}
        </div>
      )}
    </div>
  );
}
