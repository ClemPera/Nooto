import { useDraggable, useDroppable } from "@dnd-kit/core";
import { Note } from "../../types";
import Icon from "../icons/Icon";

export type TreeCallbacks = {
  onSelectNote: (id: string) => void;
  onToggleFolder: (note: Note) => void;
  onCreateNote: (parentId: string | null) => void;
  onCreateFolder: (parentId: string | null) => void;
  onDelete: (id: string) => void;
  onRestore: (id: string) => void;
  onCloseSidebar: () => void;
};

type NoteTreeItemProps = {
  note: Note;
  level: number;
  isSearchResult?: boolean;
  currentNoteId: string | null;
  showDeleted: boolean;
  filteredNotes: Note[];
  callbacks: TreeCallbacks;
};

export function buildTree(nodes: Note[], parentId: string | null): Note[] {
  return nodes
    .filter((n) => n.parent_id === parentId)
    .sort((a, b) => {
      if (a.is_folder && !b.is_folder) return -1;
      if (!a.is_folder && b.is_folder) return 1;
      return a.title.localeCompare(b.title);
    });
}

export function DragGhostItem({ note }: { note: Note }) {
  return (
    <div className="flex items-center gap-2 px-3 py-2 bg-slate-700 rounded-lg shadow-xl border border-slate-600 text-sm font-medium text-white opacity-95 max-w-[220px]">
      <Icon
        name={note.is_folder ? "folder" : "document"}
        className={`w-4 h-4 shrink-0 ${note.is_folder ? "text-blue-400" : "text-slate-400"}`}
      />
      <span className="truncate">{note.title}</span>
    </div>
  );
}

export function RootDropZone({ isDragging }: { isDragging: boolean }) {
  const { isOver, setNodeRef } = useDroppable({ id: "__root__" });
  return (
    <div
      ref={setNodeRef}
      className={`mx-1 rounded-md border border-dashed transition-all duration-150 ${
        isOver
          ? "h-8 bg-blue-400/20 border-blue-400/50 mb-1"
          : isDragging
          ? "h-6 border-slate-600 mb-1"
          : "h-0 border-transparent"
      }`}
    />
  );
}

export function NoteTreeItem({
  note,
  level,
  isSearchResult = false,
  currentNoteId,
  showDeleted,
  filteredNotes,
  callbacks,
}: NoteTreeItemProps) {
  const isActive = currentNoteId === note.id;
  const children = buildTree(filteredNotes, note.id);

  const draggable = useDraggable({
    id: note.id,
    disabled: showDeleted || isSearchResult,
  });

  const droppable = useDroppable({
    id: note.id,
    data: { isFolder: note.is_folder },
    disabled: !note.is_folder || showDeleted,
  });

  const setRef = (el: HTMLElement | null) => {
    draggable.setNodeRef(el);
    droppable.setNodeRef(el);
  };

  return (
    <div className="flex flex-col" style={{ opacity: draggable.isDragging ? 0.4 : 1 }}>
      <div
        ref={setRef}
        className={`group relative min-h-10 w-full rounded-lg text-left transition-all duration-150 flex items-center ${
          isActive
            ? "bg-slate-700 shadow-md"
            : droppable.isOver
            ? "bg-blue-500/20 ring-1 ring-blue-400/40"
            : "bg-slate-700/25 hover:bg-slate-700/50"
        }`}
        style={{ paddingLeft: "8px" }}
        {...(!showDeleted && !isSearchResult ? draggable.attributes : {})}
        {...(!showDeleted && !isSearchResult ? draggable.listeners : {})}
      >
        {/* Active accent bar */}
        <div
          className={`absolute left-0 top-1/2 -translate-y-1/2 w-0.5 rounded-full transition-all duration-200 ${
            isActive
              ? showDeleted ? "h-5 bg-red-400" : "h-5 bg-blue-400"
              : "h-0 bg-transparent"
          }`}
        />

        <div className="flex items-center flex-1 min-w-0 py-1.5">
          {!isSearchResult && note.is_folder ? (
            <button
              onClick={(e) => { e.stopPropagation(); callbacks.onToggleFolder(note); }}
              className="p-1 hover:bg-slate-600 rounded transition-colors mr-1 shrink-0"
            >
              <Icon
                name="chevronRight"
                className={`w-3.5 h-3.5 text-slate-400 transition-transform ${note.folder_open ? "rotate-90" : ""}`}
              />
            </button>
          ) : !isSearchResult ? (
            <div className="w-1 shrink-0" />
          ) : null}

          <button
            onClick={() => { callbacks.onSelectNote(note.id); callbacks.onCloseSidebar(); }}
            className="flex-1 text-left min-w-0 flex items-center gap-2 pr-2"
          >
            <Icon
              name={note.is_folder ? "folder" : "document"}
              className={`w-4 h-4 shrink-0 ${note.is_folder ? "text-blue-400" : "text-slate-400"}`}
            />
            <span
              className={`text-sm font-medium truncate transition-colors ${
                isActive ? "text-white" : "text-slate-300 group-hover:text-white"
              } ${showDeleted ? "line-through text-slate-400" : ""}`}
            >
              {note.title}
            </span>
          </button>
        </div>

        {!showDeleted && (
          <div className="flex items-center mr-1">
            {note.is_folder && (
              <>
                <button
                  onClick={(e) => { e.stopPropagation(); callbacks.onCreateNote(note.id); }}
                  className="p-1 rounded-md text-slate-500 hover:text-white hover:bg-slate-600 transition-all"
                  title="New note"
                >
                  <Icon name="plus" className="w-3.5 h-3.5" />
                </button>
                <button
                  onClick={(e) => { e.stopPropagation(); callbacks.onCreateFolder(note.id); }}
                  className="p-1 rounded-md text-slate-500 hover:text-white hover:bg-slate-600 transition-all"
                  title="New folder"
                >
                  <Icon name="folder" className="w-3.5 h-3.5" />
                </button>
              </>
            )}
            <button
              onClick={(e) => { e.stopPropagation(); callbacks.onDelete(note.id); }}
              className="p-1 rounded-md transition-all duration-150 text-slate-500 hover:text-red-400 hover:bg-red-400/10"
              title="Delete"
            >
              <Icon name="trash" className="w-3.5 h-3.5" />
            </button>
          </div>
        )}

        {showDeleted && (
          <button
            onClick={(e) => { e.stopPropagation(); callbacks.onRestore(note.id); }}
            className="shrink-0 mr-2 p-1.5 rounded-md text-slate-500 hover:text-emerald-400 hover:bg-emerald-400/10"
            title="Restore"
          >
            <Icon name="restore" className="w-3.5 h-3.5" />
          </button>
        )}
      </div>

      {!isSearchResult && note.is_folder && note.folder_open && children.length > 0 && (
        <div className="flex flex-col space-y-0.5 pl-3 border-l border-slate-700 mt-1 ml-3">
          {children.map((child) => (
            <NoteTreeItem
              key={child.id}
              note={child}
              level={level + 1}
              currentNoteId={currentNoteId}
              showDeleted={showDeleted}
              filteredNotes={filteredNotes}
              callbacks={callbacks}
            />
          ))}
        </div>
      )}
    </div>
  );
}
