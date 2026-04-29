import { NoteContent } from "../../types";
import Icon from "../icons/Icon";

type Props = {
  note: NoteContent;
  onOpenSidebar: () => void;
  onEditTitle: (title: string) => void;
  onDelete: () => void;
  onRestore: () => void;
};

export default function NoteHeader({ note, onOpenSidebar, onEditTitle, onDelete, onRestore }: Props) {
  return (
    <div className="border-b border-slate-700 p-3 md:px-6 md:py-4">
      <div className="flex items-center gap-3 overflow-hidden">
        <button
          onClick={onOpenSidebar}
          className="lg:hidden shrink-0 p-2 -ml-2 text-slate-400 hover:text-white transition-colors"
          title="Menu"
        >
          <Icon name="menu" className="w-5 h-5" />
        </button>

        <input
          type="text"
          onChange={(e) => onEditTitle(e.target.value)}
          value={note.title}
          disabled={note.deleted}
          className="flex-1 w-0 min-w-0 text-xl md:text-2xl font-bold bg-transparent text-white border-none focus:outline-none placeholder-slate-600 disabled:opacity-60 disabled:cursor-not-allowed truncate"
          placeholder={note.is_folder ? "Folder title..." : "Note title..."}
        />

        <div className="ml-auto shrink-0 flex items-center gap-3">
          {note.deleted ? (
            <button
              onClick={onRestore}
              className="text-xs font-medium text-emerald-400 bg-emerald-400/10 border border-emerald-400/20 hover:bg-emerald-400/20 px-2.5 py-1.5 rounded-md transition-colors flex items-center gap-1.5"
            >
              <Icon name="restore" className="w-3.5 h-3.5" />
              Restore
            </button>
          ) : (
            <button
              onClick={onDelete}
              className="text-xs font-medium text-red-400/80 bg-red-400/10 border border-red-400/20 hover:bg-red-400/20 px-2.5 py-1.5 rounded-md transition-colors flex items-center gap-1.5"
            >
              <Icon name="trash" className="w-3.5 h-3.5" />
              Delete
            </button>
          )}

          <div className="hidden sm:flex flex-col items-end text-[10px] text-slate-500 font-medium uppercase tracking-wider leading-none gap-1">
            <span>Last Edit</span>
            <span className="text-slate-400">{new Date(note.updated_at).toLocaleString()}</span>
          </div>
        </div>
      </div>
    </div>
  );
}
