import { useEffect, type ReactNode } from "react";
import { Loader2, X } from "lucide-react";

type Variant = "primary" | "secondary" | "ghost" | "danger";

export function Button({
  children,
  onClick,
  variant = "secondary",
  size = "md",
  disabled,
  title,
  className = "",
  autoFocus,
  type = "button",
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: Variant;
  size?: "sm" | "md" | "lg";
  disabled?: boolean;
  title?: string;
  className?: string;
  autoFocus?: boolean;
  type?: "button" | "submit";
}) {
  const base = "inline-flex items-center justify-center gap-2 rounded-lg font-semibold transition active:scale-[.98] disabled:opacity-40 disabled:pointer-events-none whitespace-nowrap";
  const sizes = { sm: "h-9 px-3 text-sm", md: "h-11 px-5 text-[15px]", lg: "h-13 px-7 text-lg" };
  const variants: Record<Variant, string> = {
    primary: "bg-white text-black hover:bg-white/85",
    secondary: "bg-white/12 text-white hover:bg-white/20",
    ghost: "bg-transparent text-white/85 hover:bg-white/10",
    danger: "bg-brand text-white hover:bg-brand2",
  };
  return (
    <button type={type} autoFocus={autoFocus} title={title} disabled={disabled} onClick={onClick} className={`${base} ${sizes[size]} ${variants[variant]} ${className}`}>
      {children}
    </button>
  );
}

export function IconButton({ children, onClick, title, className = "", active }: { children: ReactNode; onClick?: () => void; title: string; className?: string; active?: boolean }) {
  return (
    <button
      type="button"
      title={title}
      aria-label={title}
      onClick={onClick}
      className={`inline-flex h-10 w-10 items-center justify-center rounded-full transition hover:bg-white/15 active:scale-95 ${active ? "text-brand2" : "text-white"} ${className}`}
    >
      {children}
    </button>
  );
}

export function Spinner({ size = 28, className = "" }: { size?: number; className?: string }) {
  return <Loader2 size={size} className={`animate-spin text-white/80 ${className}`} />;
}

export function EmptyState({ icon, title, text, action }: { icon: ReactNode; title: string; text?: string; action?: ReactNode }) {
  return (
    <div className="fade-in mx-auto flex max-w-md flex-col items-center py-20 text-center">
      <div className="mb-5 flex h-20 w-20 items-center justify-center rounded-full bg-white/8 text-white/70">{icon}</div>
      <h2 className="text-xl font-bold">{title}</h2>
      {text && <p className="mt-2 text-white/60">{text}</p>}
      {action && <div className="mt-6">{action}</div>}
    </div>
  );
}

export function ErrorState({ text, onRetry }: { text: string; onRetry?: () => void }) {
  return (
    <div className="fade-in mx-auto flex max-w-lg flex-col items-center py-16 text-center">
      <div className="mb-4 text-5xl">😕</div>
      <p className="text-lg text-white/80">{text}</p>
      {onRetry && (
        <Button variant="primary" className="mt-6" onClick={onRetry}>
          Try again
        </Button>
      )}
    </div>
  );
}

export function SkeletonRow({ count = 7 }: { count?: number }) {
  return (
    <div className="mb-8">
      <div className="skeleton mb-3 h-6 w-48 rounded" />
      <div className="flex gap-3 overflow-hidden">
        {Array.from({ length: count }).map((_, i) => (
          <div key={i} className="skeleton aspect-[2/3] w-[160px] shrink-0 rounded-lg" />
        ))}
      </div>
    </div>
  );
}

export function SkeletonGrid({ count = 18 }: { count?: number }) {
  return (
    <div className="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-4">
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="skeleton aspect-[2/3] rounded-lg" />
      ))}
    </div>
  );
}

export function Modal({ open, onClose, title, children, width = 480 }: { open: boolean; onClose: () => void; title?: string; children: ReactNode; width?: number }) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div className="fixed inset-0 z-[80] flex items-center justify-center bg-black/70 p-4 backdrop-blur-sm" onMouseDown={onClose}>
      <div
        role="dialog"
        aria-modal="true"
        className="fade-in relative max-h-[90vh] w-full overflow-auto rounded-2xl border border-white/10 bg-panel p-6 shadow-2xl"
        style={{ maxWidth: width }}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <button className="absolute right-3 top-3 rounded-full p-2 text-white/60 hover:bg-white/10 hover:text-white" onClick={onClose} title="Close" aria-label="Close">
          <X size={20} />
        </button>
        {title && <h2 className="mb-4 pr-8 text-xl font-bold">{title}</h2>}
        {children}
      </div>
    </div>
  );
}

export function ConfirmDialog({
  open,
  title,
  text,
  confirmLabel = "OK",
  cancelLabel = "Cancel",
  danger,
  onConfirm,
  onClose,
  children,
}: {
  open: boolean;
  title: string;
  text?: ReactNode;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onClose: () => void;
  children?: ReactNode;
}) {
  return (
    <Modal open={open} onClose={onClose} title={title}>
      {text && <div className="text-white/75">{text}</div>}
      {children}
      <div className="mt-6 flex justify-end gap-3">
        <Button variant="ghost" onClick={onClose}>
          {cancelLabel}
        </Button>
        <Button
          autoFocus
          variant={danger ? "danger" : "primary"}
          onClick={() => {
            onConfirm();
            onClose();
          }}
        >
          {confirmLabel}
        </Button>
      </div>
    </Modal>
  );
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={`relative h-7 w-12 shrink-0 rounded-full transition ${checked ? "bg-brand" : "bg-white/20"}`}
    >
      <span className={`absolute top-1 h-5 w-5 rounded-full bg-white transition-all ${checked ? "left-6" : "left-1"}`} />
    </button>
  );
}

export function PageHeader({ title, subtitle, right }: { title: string; subtitle?: string; right?: ReactNode }) {
  return (
    <div className="mb-6 flex items-end justify-between gap-4">
      <div>
        <h1 className="text-3xl font-extrabold tracking-tight">{title}</h1>
        {subtitle && <p className="mt-1 text-white/55">{subtitle}</p>}
      </div>
      {right}
    </div>
  );
}
