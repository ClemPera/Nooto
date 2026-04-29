import { useToasts, Toast } from "../store/toasts";
import { ErrorKind } from "../lib/errors";

const KIND_CONFIG: Record<
  ErrorKind,
  { accent: string; bg: string; icon: JSX.Element; label: string }
> = {
  internal: {
    accent: "bg-red-500",
    bg: "border-red-500/20",
    label: "Error",
    icon: (
      <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
          d="M12 9v2m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
      </svg>
    ),
  },
  unauthorized: {
    accent: "bg-orange-500",
    bg: "border-orange-500/20",
    label: "Unauthorized",
    icon: (
      <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
          d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z" />
      </svg>
    ),
  },
  network: {
    accent: "bg-blue-500",
    bg: "border-blue-500/20",
    label: "Network",
    icon: (
      <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
          d="M8.111 16.404a5.5 5.5 0 017.778 0M12 20h.01m-7.08-7.071c3.904-3.905 10.236-3.905 14.141 0M1.394 9.393c5.857-5.857 15.355-5.857 21.213 0" />
      </svg>
    ),
  },
  not_found: {
    accent: "bg-yellow-500",
    bg: "border-yellow-500/20",
    label: "Not found",
    icon: (
      <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
          d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
      </svg>
    ),
  },
  invalid_input: {
    accent: "bg-yellow-500",
    bg: "border-yellow-500/20",
    label: "Invalid input",
    icon: (
      <svg className="w-4 h-4 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
          d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
      </svg>
    ),
  },
};

function ToastItem({ toast }: { toast: Toast }) {
  const { dismissToast } = useToasts();
  const config = KIND_CONFIG[toast.kind];

  return (
    <div
      className={`
        flex items-start gap-3 w-full
        bg-slate-800 border ${config.bg} border-l-0
        rounded-xl shadow-2xl overflow-hidden
        transition-all duration-300 ease-out
        ${toast.dismissing
          ? "opacity-0 translate-y-2 scale-95"
          : "opacity-100 translate-y-0 scale-100"
        }
      `}
      role="alert"
      aria-live="polite"
    >
      {/* Colored left accent bar */}
      <div className={`w-1 shrink-0 self-stretch rounded-l-xl ${config.accent}`} />

      {/* Icon + content */}
      <div className="flex items-start gap-2.5 flex-1 min-w-0 py-3 pr-1">
        <span className={`mt-0.5 ${config.accent.replace("bg-", "text-")}`}>
          {config.icon}
        </span>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-semibold text-slate-300 uppercase tracking-wider mb-0.5">
            {config.label}
          </p>
          <p className="text-sm text-slate-200 leading-snug break-words">
            {toast.message}
          </p>
        </div>
      </div>

      {/* Dismiss button */}
      <button
        onClick={() => dismissToast(toast.id)}
        className="shrink-0 p-2.5 mt-1 mr-1 text-slate-500 hover:text-slate-200 transition-colors rounded-lg hover:bg-slate-700/50"
        aria-label="Dismiss"
      >
        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2.5} d="M6 18L18 6M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}

export default function Toaster() {
  const { toasts } = useToasts();

  if (toasts.length === 0) return null;

  return (
    <div
      className="
        fixed bottom-6 left-1/2 -translate-x-1/2
        z-[9999]
        flex flex-col-reverse gap-2
        w-[calc(100vw-2rem)] max-w-sm
        pb-[env(safe-area-inset-bottom)]
        pointer-events-none
      "
      aria-label="Notifications"
    >
      {toasts.map((toast) => (
        <div key={toast.id} className="pointer-events-auto w-full">
          <ToastItem toast={toast} />
        </div>
      ))}
    </div>
  );
}
