import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Copy, Pencil } from "lucide-react";

export default function ModelCardMenu({
  open,
  anchorRef,
  onClose,
  children,
}: {
  open: boolean;
  anchorRef: React.RefObject<HTMLElement | null>;
  onClose: () => void;
  children: React.ReactNode;
}) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [style, setStyle] = useState<React.CSSProperties>({});

  useLayoutEffect(() => {
    if (!open || !anchorRef.current) return;
    const rect = anchorRef.current.getBoundingClientRect();
    setStyle({
      position: "fixed",
      top: rect.bottom + 4,
      left: Math.max(8, rect.right - 160),
      zIndex: 120,
    });
  }, [open, anchorRef]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);

  if (!open) return null;

  return createPortal(
    <div
      ref={menuRef}
      className="suna-model-card-dropdown is-open suna-model-card-dropdown-portal"
      style={style}
      role="menu"
    >
      {children}
    </div>,
    document.body,
  );
}

export function ModelCardMenuButton({
  icon,
  label,
  onClick,
  disabled,
  danger,
}: {
  icon?: React.ReactNode;
  label: string;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
}) {
  return (
    <button type="button" className={danger ? "is-danger" : ""} disabled={disabled} onClick={onClick}>
      {icon}
      {label}
    </button>
  );
}

export const menuIcons = { edit: <Pencil size={14} />, copy: <Copy size={14} /> };
