import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useModals } from "../store/modals";
import { NoteContent } from "./Home";

type DiffLine = {
  text: string;
  type: "same" | "changed";
};

function computeDiff(local: string, server: string): { local: DiffLine[]; server: DiffLine[] } {
  const localLines = local.split("\n");
  const serverLines = server.split("\n");
  const maxLen = Math.max(localLines.length, serverLines.length);

  const localDiff: DiffLine[] = [];
  const serverDiff: DiffLine[] = [];

  for (let i = 0; i < maxLen; i++) {
    const l = localLines[i] ?? "";
    const s = serverLines[i] ?? "";
    const type = l === s ? "same" : "changed";
    localDiff.push({ text: l, type });
    serverDiff.push({ text: s, type });
  }

  return { local: localDiff, server: serverDiff };
}

export default function ConflictModal() {
  const { conflictNote, setConflictNote } = useModals();
  const [localNote, setLocalNote] = useState<NoteContent | null>(null);
  const [resolving, setResolving] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    listen<NoteContent>("conflict", (event) => {
      const serverNote = event.payload;
      setConflictNote(serverNote);

      invoke("get_note", { id: serverNote.id })
        .then((note) => setLocalNote(note as NoteContent))
        .catch((e) => console.error(e));
    }).then((fn) => { unlisten = fn; });

    return () => { unlisten?.(); };
  }, []);

  async function handleResolve(keepLocal: boolean) {
    if (!conflictNote) return;
    setResolving(true);

    await invoke("handle_conflict", { id: conflictNote.id, local: keepLocal })
      .catch((e) => console.error(e));

    setConflictNote(null);
    setLocalNote(null);
    setResolving(false);
  }

  if (!conflictNote || !localNote) return null;

  const diff = computeDiff(localNote.content, conflictNote.content);
  const localDate = new Date(localNote.updated_at).toLocaleString();
  const serverDate = new Date(conflictNote.updated_at).toLocaleString();

  return (
    <div className="min-h-screen min-w-screen pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] flex items-center justify-center p-4 fixed z-50">
      <div className="fixed inset-0 backdrop-blur-sm bg-black/40" />

      <div className="relative bg-slate-800 rounded-2xl shadow-2xl w-full max-w-5xl flex flex-col max-h-[90vh]">

        {/* Header */}
        <div className="p-6 border-b border-slate-700 shrink-0">
          <div className="flex items-center gap-3 mb-1">
            <div className="w-8 h-8 bg-amber-500/20 rounded-full flex items-center justify-center shrink-0">
              <svg className="w-4 h-4 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 9v2m0 4h.01M10.29 3.86L1.82 18a2 2 0 001.71 3h16.94a2 2 0 001.71-3L13.71 3.86a2 2 0 00-3.42 0z" />
              </svg>
            </div>
            <h2 className="text-lg font-bold text-white">Sync Conflict</h2>
          </div>
          <p className="text-slate-400 text-sm ml-11">
            <span className="font-medium text-white">{conflictNote.title}</span> was modified on another device. Choose which version to keep.
          </p>
        </div>

        {/* Diff area */}
        <div className="flex flex-1 overflow-hidden divide-x divide-slate-700 min-h-0">

          {/* Local version */}
          <div className="flex-1 flex flex-col overflow-hidden">
            <div className="px-4 py-2.5 bg-slate-700/40 border-b border-slate-700 shrink-0 flex items-center justify-between">
              <span className="text-xs font-semibold text-blue-400 uppercase tracking-wider">Local</span>
              <span className="text-xs text-slate-500">{localDate}</span>
            </div>
            <div className="flex-1 overflow-y-auto font-mono text-xs">
              {diff.local.map((line, i) => (
                <div
                  key={i}
                  className={`px-4 py-0.5 whitespace-pre-wrap break-all leading-5 ${line.type === "changed"
                    ? "bg-blue-500/10 text-blue-200"
                    : "text-slate-300"
                    }`}
                >
                  {line.text || "\u00A0"}
                </div>
              ))}
            </div>
          </div>

          {/* Server version */}
          <div className="flex-1 flex flex-col overflow-hidden">
            <div className="px-4 py-2.5 bg-slate-700/40 border-b border-slate-700 shrink-0 flex items-center justify-between">
              <span className="text-xs font-semibold text-emerald-400 uppercase tracking-wider">Server</span>
              <span className="text-xs text-slate-500">{serverDate}</span>
            </div>
            <div className="flex-1 overflow-y-auto font-mono text-xs">
              {diff.server.map((line, i) => (
                <div
                  key={i}
                  className={`px-4 py-0.5 whitespace-pre-wrap break-all leading-5 ${line.type === "changed"
                    ? "bg-emerald-500/10 text-emerald-200"
                    : "text-slate-300"
                    }`}
                >
                  {line.text || "\u00A0"}
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* Actions */}
        <div className="p-4 border-t border-slate-700 flex gap-3 shrink-0">
          <button
            onClick={() => handleResolve(true)}
            disabled={resolving}
            className="flex-1 px-4 py-2.5 bg-blue-600 hover:bg-blue-700 disabled:opacity-50 text-white font-semibold rounded-lg transition-colors text-sm"
          >
            Keep Local
          </button>
          <button
            onClick={() => handleResolve(false)}
            disabled={resolving}
            className="flex-1 px-4 py-2.5 bg-emerald-600 hover:bg-emerald-700 disabled:opacity-50 text-white font-semibold rounded-lg transition-colors text-sm"
          >
            Keep Server
          </button>
        </div>
      </div>
    </div>
  );
}
