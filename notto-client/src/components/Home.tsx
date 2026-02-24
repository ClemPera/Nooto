import { useEffect, useState } from "react";
import { useGeneral, Note } from "../store/general";
import { useModals } from "../store/modals";
import { invoke } from "@tauri-apps/api/core";
import AccountMenu from "./AccountMenu";
import { trace } from "@tauri-apps/plugin-log";
import { listen } from "@tauri-apps/api/event";

type NoteContent = {
  id: string;
  title: string;
  content: string;
  updated_at: Date;
  deleted: boolean;
};

export default function Home() {
  const { workspace, notes, setNotes } = useGeneral();
  const { setShowDeleteNoteConfirm } = useModals();
  const [currentNote, setCurrentNote] = useState<NoteContent | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [showDeleted, setShowDeleted] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    listen<Note[]>('new_note_metadata', (event) => {
      setNotes(event.payload)

      if (currentNote) {
        const note = notes?.find(note => note.id == currentNote.id)

        if (!note) {
          //Current note is probably deleted
          //TODO: handle.

        } else if (note.updated_at != currentNote.updated_at) {
          //Note has been modified
          get_note(note.id)
        }
      }
    }).then(unlistenFn => {
      unlisten = unlistenFn;
    });

    return () => {
      if (unlisten)
        unlisten()
    }
  }, [currentNote])

  useEffect(() => {
    get_notes_metadata();
  }, [workspace]);

  // Clear currentNote if it was removed from the notes list (e.g. deleted via modal)
  useEffect(() => {
    if (currentNote && !notes.find((n) => n.id === currentNote.id)) {
      setCurrentNote(null);
    }
  }, [notes]);

  // When switching tabs, clear current note if it doesn't belong to the new tab
  useEffect(() => {
    if (currentNote) {
      const noteInList = notes.find((n) => n.id === currentNote.id);
      if (noteInList && noteInList.deleted !== showDeleted) {
        setCurrentNote(null);
      }
    }
  }, [showDeleted]);

  function get_notes_metadata() {
    if (workspace) {
      trace("getting notes metadata from: " + workspace?.id + " - " + workspace?.workspace_name)
      invoke("get_all_notes_metadata", { id_workspace: workspace?.id })
        .then((fetched) => setNotes(fetched as Note[]))
        .catch((e) => console.error(e));
    }
  }

  async function create_note() {
    await invoke("create_note", { title: "New Note" }).catch((e) =>
      console.error(e)
    );
    get_notes_metadata();
  }

  async function get_note(id: string) {
    await invoke("get_note", { id: id })
      .then((note) => {
        setCurrentNote(note as NoteContent);
        trace!("note received: " + (note as NoteContent).id)
      })
      .catch((e) => console.error(e));
    setSidebarOpen(false); // Close sidebar on mobile after selecting note
  }

  async function edit_note(content: string) {
    const note: NoteContent = {
      id: currentNote?.id!,
      title: currentNote?.title!,
      updated_at: currentNote?.updated_at!,
      content: content,
      deleted: currentNote?.deleted!,
    };

    setCurrentNote(note);

    invoke("edit_note", { note }).catch((e) => console.error(e));
  }

  async function restore_note(id: string) {
    await invoke("restore_note", { id }).catch((e) => console.error(e));
    setCurrentNote(null);
    get_notes_metadata();
  }

  async function edit_note_title(title: string) {
    const note: NoteContent = {
      id: currentNote?.id!,
      title: title!,
      updated_at: currentNote?.updated_at!,
      content: currentNote?.content!,
      deleted: currentNote?.deleted!,
    };

    setCurrentNote(note);

    await invoke("edit_note", { note }).catch((e) => console.error(e));

    get_notes_metadata();
  }

  const filteredNotes = notes.filter((note) =>
    note.deleted === showDeleted &&
    note.title.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const deletedCount = notes.filter((n) => n.deleted).length;

  return (
    <div className="flex h-screen pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] bg-slate-900 overflow-hidden overscroll-none">
      {/* Mobile overlay */}
      {sidebarOpen && (
        <div
          className="fixed inset-0 bg-black/50 z-40 lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      {/* Sidebar */}
      <div className={`
        fixed lg:relative inset-y-0 left-0 z-40 pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]
        w-80 bg-slate-800 border-r border-slate-700 flex flex-col
        transform transition-transform duration-200 ease-in-out
        ${sidebarOpen ? 'translate-x-0' : '-translate-x-full lg:translate-x-0'}
      `}>
        {/* Sidebar Header */}
        <div className="p-3 md:p-4 border-b border-slate-700">
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-xl font-bold text-white">Notto</h2>
            <div className="flex gap-2">
              {!showDeleted && (
                <button
                  onClick={create_note}
                  className="p-2 bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors"
                  title="New Note"
                >
                  <svg
                    className="w-5 h-5 text-white"
                    fill="none"
                    stroke="currentColor"
                    viewBox="0 0 24 24"
                  >
                    <path
                      strokeLinecap="round"
                      strokeLinejoin="round"
                      strokeWidth={2}
                      d="M12 4v16m8-8H4"
                    />
                  </svg>
                </button>
              )}
              <button
                onClick={() => setSidebarOpen(false)}
                className="p-2 lg:hidden text-slate-400 hover:text-white transition-colors"
                title="Close"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
          </div>

          {/* Active / Deleted toggle */}
          <div className="flex rounded-lg overflow-hidden border border-slate-600 mb-3 text-sm font-medium">
            <button
              onClick={() => setShowDeleted(false)}
              className={`flex-1 py-1.5 transition-colors ${
                !showDeleted
                  ? "bg-slate-600 text-white"
                  : "text-slate-400 hover:text-slate-200 hover:bg-slate-700/50"
              }`}
            >
              Notes
            </button>
            <button
              onClick={() => setShowDeleted(true)}
              className={`flex-1 py-1.5 transition-colors flex items-center justify-center gap-1.5 ${
                showDeleted
                  ? "bg-slate-600 text-white"
                  : "text-slate-400 hover:text-slate-200 hover:bg-slate-700/50"
              }`}
            >
              Trash
              {deletedCount > 0 && (
                <span className={`text-xs px-1.5 py-0.5 rounded-full font-semibold ${
                  showDeleted ? "bg-red-500/30 text-red-300" : "bg-slate-500/50 text-slate-400"
                }`}>
                  {deletedCount}
                </span>
              )}
            </button>
          </div>

          {/* Search */}
          <input
            type="text"
            placeholder={showDeleted ? "Search trash..." : "Search notes..."}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-white placeholder-slate-400 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          />
        </div>

        {/* Notes List */}
        <div className="flex-1 overflow-y-auto">
          {filteredNotes && filteredNotes.length > 0 ? (
              <div className="px-2 py-1 divide-y space-y-1 divide-slate-700/60">
              {filteredNotes.map((note) => {
                const isActive = currentNote?.id === note.id;
                return (
                  <div
                    key={note.id}
                    className={`group relative w-full rounded-lg text-left transition-all duration-150 flex items-center ${isActive
                        ? "bg-slate-700 shadow-md"
                        : "bg-slate-700/25 hover:bg-slate-700/50"
                      }`}
                  >
                    {/* Active accent bar */}
                    <div
                      className={`absolute left-0 top-1/2 -translate-y-1/2 w-0.5 rounded-full transition-all duration-200 ${isActive
                          ? showDeleted ? "h-5 bg-red-400" : "h-5 bg-blue-400"
                          : "h-0 bg-transparent"
                        }`}
                    />

                    <button
                      onClick={() => get_note(note.id)}
                      className="flex-1 text-left min-w-0 px-4 py-2.5"
                    >
                      <div
                        className={`text-sm font-medium truncate transition-colors ${isActive ? "text-white" : "text-slate-300 group-hover:text-white"
                          } ${showDeleted ? "line-through text-slate-400" : ""}`}
                      >
                        {note.title}
                      </div>
                    </button>

                    {showDeleted ? (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          restore_note(note.id);
                        }}
                        className="shrink-0 mr-2 p-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-all duration-150 text-slate-500 hover:text-emerald-400 hover:bg-emerald-400/10"
                        title="Restore note"
                      >
                        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6" />
                        </svg>
                      </button>
                    ) : (
                      <button
                        onClick={(e) => {
                          e.stopPropagation();
                          setShowDeleteNoteConfirm(true, note.id);
                        }}
                        className={`shrink-0 mr-2 p-1.5 rounded-md opacity-0 group-hover:opacity-100 transition-all duration-150 ${isActive
                            ? "text-slate-400 hover:text-red-400 hover:bg-red-400/10"
                            : "text-slate-500 hover:text-red-400 hover:bg-red-400/10"
                          }`}
                        title="Delete note"
                      >
                        <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path
                            strokeLinecap="round"
                            strokeLinejoin="round"
                            strokeWidth={2}
                            d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                          />
                        </svg>
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="flex items-center justify-center h-full text-slate-500 text-sm p-4 text-center">
              {searchQuery
                ? "No notes found"
                : showDeleted
                  ? "Trash is empty"
                  : "No notes yet. Click + to create one!"}
            </div>
          )}
        </div>

        {/* Account Menu at bottom */}
        <AccountMenu />
      </div>

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col bg-slate-900">
        {currentNote ? (
          <>
            {/* Note Title with mobile menu button */}
            <div className="border-b border-slate-700 p-3 md:p-4 flex items-center gap-3 overflow-hidden">
              <button
                onClick={() => setSidebarOpen(true)}
                className="lg:hidden shrink-0 p-2 text-slate-400 hover:text-white transition-colors"
                title="Menu"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
                </svg>
              </button>
              <input
                type="text"
                onChange={(e) => edit_note_title(e.target.value)}
                value={currentNote.title}
                disabled={currentNote.deleted}
                className="flex-1 w-0 min-w-0 text-xl md:text-2xl font-bold bg-transparent text-white border-none focus:outline-none placeholder-slate-600 disabled:opacity-60 disabled:cursor-not-allowed truncate"
                placeholder="Note title..."
              />
              <div className="ml-auto shrink-0 flex items-center gap-2">
                {currentNote.deleted ? (
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-red-400/80 bg-red-400/10 border border-red-400/20 px-2 py-1 rounded-md">
                      Deleted
                    </span>
                    <button
                      onClick={() => restore_note(currentNote.id)}
                      className="text-xs text-emerald-400 bg-emerald-400/10 border border-emerald-400/20 hover:bg-emerald-400/20 px-2 py-1 rounded-md transition-colors flex items-center gap-1"
                      title="Restore note"
                    >
                      <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6" />
                      </svg>
                      <span className="hidden sm:inline">Restore</span>
                    </button>
                  </div>
                ) : (
                  <button
                    onClick={() => setShowDeleteNoteConfirm(true, currentNote.id)}
                    className="text-xs text-red-400/80 bg-red-400/10 border border-red-400/20 hover:bg-red-400/20 px-2 py-1 rounded-md transition-colors flex items-center gap-1"
                    title="Delete note"
                  >
                    <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                    </svg>
                    <span className="hidden sm:inline">Delete</span>
                  </button>
                )}
                <span className="text-xs text-slate-500 text-right leading-tight">
                  <span className="sm:hidden">
                    <span className="block">{new Date(currentNote.updated_at).toLocaleDateString()}</span>
                    <span className="block">{new Date(currentNote.updated_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
                  </span>
                  <span className="hidden sm:inline whitespace-nowrap">
                    {new Date(currentNote.updated_at).toLocaleString()}
                  </span>
                </span>
              </div>
            </div>
            {/* Note Content */}
            <div className="flex-1 p-3 md:p-4 overflow-y-auto overflow-x-hidden">
              <textarea
                onChange={(e) => edit_note(e.target.value)}
                value={currentNote.content}
                disabled={currentNote.deleted}
                className="w-full h-full bg-transparent text-white resize-none border-none focus:outline-none placeholder-slate-600 text-sm md:text-base leading-relaxed disabled:opacity-60 disabled:cursor-not-allowed"
                placeholder="Start writing..."
              />
            </div>
          </>
        ) : (
          <div className="flex-1 flex flex-col">
            <div className="lg:hidden border-b border-slate-700 p-3">
              <button
                onClick={() => setSidebarOpen(true)}
                className="p-2 text-slate-400 hover:text-white transition-colors"
                title="Menu"
              >
                <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 6h16M4 12h16M4 18h16" />
                </svg>
              </button>
            </div>
            <div className="flex-1 flex items-center justify-center text-slate-500 p-4">
              <div className="text-center">
                <svg
                  className="w-12 h-12 md:w-16 md:h-16 mx-auto mb-4 opacity-50"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={1.5}
                    d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
                  />
                </svg>
                <p className="text-base md:text-lg">Select a note to view</p>
                <p className="text-xs md:text-sm mt-1">Or create a new one to get started</p>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}