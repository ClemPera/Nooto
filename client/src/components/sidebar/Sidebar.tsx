import { useState } from "react";
import {
  DndContext,
  DragEndEvent,
  DragOverlay,
  DragStartEvent,
  MouseSensor,
  TouchSensor,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import { Note } from "../../types";
import { buildTree, DragGhostItem, NoteTreeItem, RootDropZone, TreeCallbacks } from "./NoteTree";
import AccountMenu from "../account/AccountMenu";
import Icon from "../icons/Icon";

type Props = {
  notes: Note[];
  currentNoteId: string | null;
  sidebarOpen: boolean;
  showDeleted: boolean;
  deletedCount: number;
  onClose: () => void;
  onToggleDeleted: (v: boolean) => void;
  onMoveItem: (id: string, parentId: string | null) => void;
  onCreateNote: (parentId: string | null) => void;
  onCreateFolder: (parentId: string | null) => void;
  callbacks: TreeCallbacks;
};

export default function Sidebar({
  notes,
  currentNoteId,
  sidebarOpen,
  showDeleted,
  deletedCount,
  onClose,
  onToggleDeleted,
  onMoveItem,
  onCreateNote,
  onCreateFolder,
  callbacks,
}: Props) {
  const [searchQuery, setSearchQuery] = useState("");
  const [dragActiveId, setDragActiveId] = useState<string | null>(null);

  // Desktop: drag starts after 8px movement; mobile: long-press 200ms
  const sensors = useSensors(
    useSensor(MouseSensor, { activationConstraint: { distance: 8 } }),
    useSensor(TouchSensor, { activationConstraint: { delay: 200, tolerance: 5 } })
  );

  function isDescendantOf(nodeId: string, ancestorId: string): boolean {
    let current = notes.find((n) => n.id === nodeId);
    while (current?.parent_id) {
      if (current.parent_id === ancestorId) return true;
      current = notes.find((n) => n.id === current!.parent_id);
    }
    return false;
  }

  function handleDragStart({ active }: DragStartEvent) {
    setDragActiveId(active.id as string);
  }

  function handleDragEnd({ active, over }: DragEndEvent) {
    setDragActiveId(null);
    if (!over || active.id === over.id) return;
    const overId = over.id as string;
    if (overId === "__root__") {
      onMoveItem(active.id as string, null);
    } else if (over.data.current?.isFolder && !isDescendantOf(overId, active.id as string)) {
      onMoveItem(active.id as string, overId);
    }
  }

  const isSearching = searchQuery !== "";

  const filteredNotes = notes.filter(
    (note) =>
      (showDeleted ? !note.is_folder : true) &&
      note.deleted === showDeleted &&
      (!isSearching || note.title.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  const dragActiveNote = dragActiveId ? notes.find((n) => n.id === dragActiveId) ?? null : null;

  return (
    <div
      className={`
        fixed lg:relative inset-y-0 left-0 z-40 pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]
        w-80 bg-slate-800 border-r border-slate-700 flex flex-col
        transform transition-transform duration-200 ease-in-out
        ${sidebarOpen ? "translate-x-0" : "-translate-x-full lg:translate-x-0"}
      `}
    >
      {/* Header */}
      <div className="p-3 md:p-4 border-b border-slate-700">
        <div className="flex items-center justify-between mb-3">
          <h2 className="text-xl font-bold text-white">Nooto</h2>
          <div className="flex gap-1">
            {!showDeleted && (
              <>
                <button
                  onClick={() => onCreateNote(null)}
                  className="p-2 bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors"
                  title="New Note"
                >
                  <Icon name="plus" className="w-5 h-5 text-white" />
                </button>
                <button
                  onClick={() => onCreateFolder(null)}
                  className="p-2 bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors"
                  title="New Folder"
                >
                  <Icon name="folder" className="w-5 h-5 text-white" />
                </button>
              </>
            )}
            <button
              onClick={onClose}
              className="p-2 lg:hidden text-slate-400 hover:text-white transition-colors"
              title="Close"
            >
              <Icon name="close" className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Notes / Trash toggle */}
        <div className="flex rounded-lg overflow-hidden border border-slate-600 mb-3 text-sm font-medium">
          <button
            onClick={() => onToggleDeleted(false)}
            className={`flex-1 py-1.5 transition-colors ${
              !showDeleted ? "bg-slate-600 text-white" : "text-slate-400 hover:text-slate-200 hover:bg-slate-700/50"
            }`}
          >
            Notes
          </button>
          <button
            onClick={() => onToggleDeleted(true)}
            className={`flex-1 py-1.5 transition-colors flex items-center justify-center gap-1.5 ${
              showDeleted ? "bg-slate-600 text-white" : "text-slate-400 hover:text-slate-200 hover:bg-slate-700/50"
            }`}
          >
            Trash
            {deletedCount > 0 && (
              <span
                className={`text-xs px-1.5 py-0.5 rounded-full font-semibold ${
                  showDeleted ? "bg-red-500/30 text-red-300" : "bg-slate-500/50 text-slate-400"
                }`}
              >
                {deletedCount}
              </span>
            )}
          </button>
        </div>

        <input
          type="text"
          placeholder={showDeleted ? "Search trash..." : "Search notes..."}
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          className="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-white placeholder-slate-400 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
        />
      </div>

      {/* Note list */}
      <div className="flex-1 overflow-y-auto">
        <DndContext sensors={sensors} onDragStart={handleDragStart} onDragEnd={handleDragEnd}>
          <div className="px-2 py-2 space-y-0.5">
            {!showDeleted && !isSearching && <RootDropZone isDragging={dragActiveId !== null} />}

            {showDeleted || isSearching
              ? filteredNotes.map((note) => (
                  <NoteTreeItem
                    key={note.id}
                    note={note}
                    level={0}
                    isSearchResult
                    currentNoteId={currentNoteId}
                    showDeleted={showDeleted}
                    filteredNotes={filteredNotes}
                    callbacks={callbacks}
                  />
                ))
              : buildTree(filteredNotes, null).map((note) => (
                  <NoteTreeItem
                    key={note.id}
                    note={note}
                    level={0}
                    currentNoteId={currentNoteId}
                    showDeleted={showDeleted}
                    filteredNotes={filteredNotes}
                    callbacks={callbacks}
                  />
                ))}

            {filteredNotes.length === 0 && (
              <div className="flex items-center justify-center h-full text-slate-500 text-sm p-4 text-center mt-10">
                {isSearching
                  ? "No results found"
                  : showDeleted
                  ? "Trash is empty"
                  : "No notes yet. Click + to create one!"}
              </div>
            )}
          </div>

          <DragOverlay>
            {dragActiveNote && <DragGhostItem note={dragActiveNote} />}
          </DragOverlay>
        </DndContext>
      </div>

      <AccountMenu />
    </div>
  );
}
