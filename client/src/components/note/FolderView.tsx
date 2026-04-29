import { Note } from "../../types";
import Icon from "../icons/Icon";

type Props = {
  folderId: string;
  notes: Note[];
  onSelect: (id: string) => void;
};

export default function FolderView({ folderId, notes, onSelect }: Props) {
  const children = notes
    .filter((n) => n.parent_id === folderId && !n.deleted)
    .sort((a, b) => {
      if (a.is_folder && !b.is_folder) return -1;
      if (!a.is_folder && b.is_folder) return 1;
      return a.title.localeCompare(b.title);
    });

  if (children.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-slate-500 opacity-40 select-none p-6">
        <div className="p-8 bg-slate-800/50 rounded-full mb-6">
          <Icon name="folder" className="w-16 h-16" strokeWidth={1} />
        </div>
        <p className="text-sm text-center max-w-xs">This folder is empty.</p>
      </div>
    );
  }

  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3 content-start p-3 md:p-6 overflow-y-auto">
      {children.map((child) => (
        <button
          key={child.id}
          onClick={() => onSelect(child.id)}
          className="flex flex-col items-start gap-2 p-3 bg-slate-800 hover:bg-slate-700 rounded-xl border border-slate-700 hover:border-slate-600 transition-all text-left group"
        >
          <Icon
            name={child.is_folder ? "folder" : "document"}
            className={`w-6 h-6 ${child.is_folder ? "text-blue-400" : "text-slate-400"}`}
            strokeWidth={1.5}
          />
          <span className="text-sm font-medium text-slate-300 group-hover:text-white transition-colors truncate w-full">
            {child.title}
          </span>
        </button>
      ))}
    </div>
  );
}
