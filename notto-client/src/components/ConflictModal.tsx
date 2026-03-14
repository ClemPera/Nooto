import { invoke } from "@tauri-apps/api/core";
import { useModals } from "../store/modals";

export default function ConflictModal() {
  const { conflictPayload, setConflictPayload } = useModals();

  async function handleResolve(keepLocal: boolean) {
    if (!conflictPayload) return;

    await invoke("resolve_conflict", {
      uuid: conflictPayload.uuid,
      keep_local: keepLocal,
    }).catch((e) => console.error(e));

    setConflictPayload(null);
  }

  if (!conflictPayload) return null;

  return (
    <div className="min-h-screen min-w-screen pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] flex items-center justify-center p-4 fixed z-50">
      <div className="fixed inset-0 backdrop-blur-sm" />

      <div className="relative bg-slate-800 rounded-2xl shadow-2xl w-full max-w-2xl p-8">
        <div className="text-center mb-6">
          <div className="mx-auto w-16 h-16 bg-yellow-100 rounded-full flex items-center justify-center mb-4">
            <svg className="w-8 h-8 text-yellow-600" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
            </svg>
          </div>
          <h2 className="text-2xl font-bold text-white mb-1">Sync Conflict</h2>
          <p className="text-slate-400 text-sm">
            This note was modified on another device. Choose which version to keep.
          </p>
        </div>

        <div className="grid grid-cols-2 gap-4 mb-6">
          <VersionCard
            label="Your version"
            title={conflictPayload.local.title}
            content={conflictPayload.local.content}
            updated_at={conflictPayload.local.updated_at}
            accentClass="border-blue-500/40 hover:border-blue-500"
            buttonClass="bg-blue-600 hover:bg-blue-700"
            onKeep={() => handleResolve(true)}
          />
          <VersionCard
            label="Server version"
            title={conflictPayload.server.title}
            content={conflictPayload.server.content}
            updated_at={conflictPayload.server.updated_at}
            accentClass="border-purple-500/40 hover:border-purple-500"
            buttonClass="bg-purple-600 hover:bg-purple-700"
            onKeep={() => handleResolve(false)}
          />
        </div>
      </div>
    </div>
  );
}

type VersionCardProps = {
  label: string;
  title: string;
  content: string;
  updated_at: number;
  accentClass: string;
  buttonClass: string;
  onKeep: () => void;
};

function VersionCard({ label, title, content, updated_at, accentClass, buttonClass, onKeep }: VersionCardProps) {
  const preview = content.trim().slice(0, 120) + (content.length > 120 ? "…" : "");

  return (
    <div className={`flex flex-col border rounded-xl p-4 bg-slate-700/50 transition-colors ${accentClass}`}>
      <span className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">{label}</span>
      <p className="text-white font-semibold truncate mb-1">{title || <span className="text-slate-500 italic">Untitled</span>}</p>
      <p className="text-slate-400 text-sm flex-1 leading-relaxed break-words line-clamp-4 mb-3">
        {preview || <span className="italic">Empty note</span>}
      </p>
      <p className="text-xs text-slate-500 mb-3">{new Date(updated_at).toLocaleString()}</p>
      <button
        onClick={onKeep}
        className={`w-full py-2 text-white text-sm font-semibold rounded-lg transition-colors ${buttonClass}`}
      >
        Keep this one
      </button>
    </div>
  );
}
