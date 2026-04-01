import { useEffect, useState } from "react";
import { useGeneral, Note } from "../store/general";
import { useModals } from "../store/modals";
import { invoke } from "@tauri-apps/api/core";
import AccountMenu from "./AccountMenu";
import { trace } from "@tauri-apps/plugin-log";
import { listen } from "@tauri-apps/api/event";

export type NoteContent = {
  id: string;
  title: string;
  parent_id: string | null;
  is_folder: boolean;
  folder_open: boolean;
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
      const notes_received = event.payload
      setNotes(notes_received)

      if (currentNote) {
        const note = notes_received?.find(note => note.id == currentNote.id)

        if (!note) {
          //Current note is probably hard deleted
          //TODO: handle.
        } else {
          if (note.updated_at != currentNote.updated_at) {
            //Note has been modified

            get_note(note.id)
          }
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
    get_latest_note();
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

  function get_latest_note() {
    if (!currentNote) {
      invoke("get_latest_note_id")
        .then((id) => { if (id as string | null) get_note(id as string) })
        .catch((e) => console.error(e));
    }
  }

  async function create_note(parent_id: string | null = null) {
    await invoke("create_note", { title: "New Note", parent_id })
      .then((uuid) => get_note(uuid as string))
      .catch((e) => console.error(e));

    get_notes_metadata();
  }

  async function create_folder(parent_id: string | null = null) {
    await invoke("create_folder", { title: "New Folder", parent_id })
      .then(() => get_notes_metadata())
      .catch((e) => console.error(e));
  }

  async function toggle_folder(note: Note) {
    invoke("get_note", { id: note.id })
      .then((fullNote: any) => {
        const noteToUpdate: NoteContent = {
          ...fullNote,
          folder_open: !note.folder_open
        };
        invoke("edit_note", { note: noteToUpdate }).then(() => get_notes_metadata());
      })
      .catch((e) => console.error(e));
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
      ...currentNote!,
      content: content,
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
      ...currentNote!,
      title: title!,
    };

    setCurrentNote(note);

    await invoke("edit_note", { note }).catch((e) => console.error(e));

    get_notes_metadata();
  }

  const filteredNotes = notes.filter((note) =>
    note.deleted === showDeleted &&
    (searchQuery === "" || note.title.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  const buildTree = (nodes: Note[], parentId: string | null = null): Note[] => {
    return nodes
      .filter((node) => node.parent_id === parentId)
      .sort((a, b) => {
        if (a.is_folder && !b.is_folder) return -1;
        if (!a.is_folder && b.is_folder) return 1;
        return a.title.localeCompare(b.title);
      });
  };

  const getPath = (nodeId: string | null): Note[] => {
    if (!nodeId) return [];
    const node = notes.find(n => n.id === nodeId);
    if (!node) return [];
    const path = node.parent_id ? getPath(node.parent_id) : [];
    return [...path, node];
  };

  const NoteTreeItem = ({ note, level = 0, isSearchResult = false }: { note: Note; level: number; isSearchResult?: boolean }) => {
    const isActive = currentNote?.id === note.id;
    const children = buildTree(filteredNotes, note.id);
    const hasChildren = children.length > 0;

    return (
      <div className="flex flex-col">
        <div
          className={`group relative min-h-10 w-full rounded-lg text-left transition-all duration-150 flex items-center ${isActive
            ? "bg-slate-700 shadow-md"
            : "bg-slate-700/25 hover:bg-slate-700/50"
            }`}
          style={{ paddingLeft: isSearchResult ? "8px" : `${level * 12 + 8}px` }}
        >
          {/* Active accent bar */}
          <div
            className={`absolute left-0 top-1/2 -translate-y-1/2 w-0.5 rounded-full transition-all duration-200 ${isActive
              ? showDeleted ? "h-5 bg-red-400" : "h-5 bg-blue-400"
              : "h-0 bg-transparent"
              }`}
          />

          <div className="flex items-center flex-1 min-w-0 py-1.5">
            {!isSearchResult && note.is_folder ? (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  toggle_folder(note);
                }}
                className="p-1 hover:bg-slate-600 rounded transition-colors mr-1"
              >
                <svg
                  className={`w-3.5 h-3.5 text-slate-400 transition-transform ${note.folder_open ? "rotate-90" : ""}`}
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                </svg>
              </button>
            ) : !isSearchResult && (
              <div className="w-5" />
            )}

            <button
              onClick={() => get_note(note.id)}
              className="flex-1 text-left min-w-0 flex items-center gap-2 pr-2"
            >
              {note.is_folder ? (
                <svg className="w-4 h-4 text-blue-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                </svg>
              ) : (
                <svg className="w-4 h-4 text-slate-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
                </svg>
              )}
              <div
                className={`text-sm font-medium truncate transition-colors ${isActive ? "text-white" : "text-slate-300 group-hover:text-white"
                  } ${showDeleted ? "line-through text-slate-400" : ""}`}
              >
                {note.title}
              </div>
            </button>
          </div>

          {!showDeleted && (
            <div className="flex items-center mr-1 opacity-0 group-hover:opacity-100 transition-opacity">
              {note.is_folder && (
                <>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      create_note(note.id);
                    }}
                    className="p-1 rounded-md text-slate-500 hover:text-white hover:bg-slate-600 transition-all"
                    title="New note"
                  >
                    <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 4v16m8-8H4" />
                    </svg>
                  </button>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      create_folder(note.id);
                    }}
                    className="p-1 rounded-md text-slate-500 hover:text-white hover:bg-slate-600 transition-all"
                    title="New folder"
                  >
                    <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                    </svg>
                  </button>
                </>
              )}
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  setShowDeleteNoteConfirm(true, note.id);
                }}
                className={`p-1 rounded-md transition-all duration-150 text-slate-500 hover:text-red-400 hover:bg-red-400/10`}
                title="Delete"
              >
                <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
              </button>
            </div>
          )}

          {showDeleted && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                restore_note(note.id);
              }}
              className="shrink-0 mr-2 p-1.5 rounded-md text-slate-500 hover:text-emerald-400 hover:bg-emerald-400/10"
              title="Restore"
            >
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6" />
              </svg>
            </button>
          )}
        </div>
        {!isSearchResult && note.is_folder && note.folder_open && hasChildren && (
          <div className="flex flex-col">
            {children.map((child) => (
              <NoteTreeItem key={child.id} note={child} level={level + 1} />
            ))}
          </div>
        )}
      </div>
    );
  };

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
            <div className="flex gap-1">
              {!showDeleted && (
                <>
                  <button
                    onClick={() => create_note(null)}
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
                  <button
                    onClick={() => create_folder(null)}
                    className="p-2 bg-slate-700 hover:bg-slate-600 rounded-lg transition-colors"
                    title="New Folder"
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
                        d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"
                      />
                    </svg>
                  </button>
                </>
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
              className={`flex-1 py-1.5 transition-colors ${!showDeleted
                ? "bg-slate-600 text-white"
                : "text-slate-400 hover:text-slate-200 hover:bg-slate-700/50"
                }`}
            >
              Notes
            </button>
            <button
              onClick={() => setShowDeleted(true)}
              className={`flex-1 py-1.5 transition-colors flex items-center justify-center gap-1.5 ${showDeleted
                ? "bg-slate-600 text-white"
                : "text-slate-400 hover:text-slate-200 hover:bg-slate-700/50"
                }`}
            >
              Trash
              {deletedCount > 0 && (
                <span className={`text-xs px-1.5 py-0.5 rounded-full font-semibold ${showDeleted ? "bg-red-500/30 text-red-300" : "bg-slate-500/50 text-slate-400"
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
          {filteredNotes && (
            <div className="px-2 py-2 space-y-0.5">
              {showDeleted || searchQuery !== "" ? (
                filteredNotes.map((note) => (
                  <NoteTreeItem key={note.id} note={note} level={0} isSearchResult={true} />
                ))
              ) : (
                buildTree(filteredNotes, null).map((note) => (
                  <NoteTreeItem key={note.id} note={note} level={0} />
                ))
              )}
              {filteredNotes.length === 0 && (
                <div className="flex items-center justify-center h-full text-slate-500 text-sm p-4 text-center mt-10">
                  {searchQuery
                    ? "No results found"
                    : showDeleted
                      ? "Trash is empty"
                      : "No notes yet. Click + to create one!"}
                </div>
              )}
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
            {/* Note Header */}
            <div className="border-b border-slate-700 p-3 md:px-6 md:py-4 flex flex-col gap-2">
              <div className="flex items-center justify-between">
                {/* Breadcrumbs */}
                <div className="flex items-center gap-1.5 text-xs font-medium text-slate-500 overflow-hidden">
                  <button 
                    onClick={() => {
                      const updatedNote = { ...currentNote, parent_id: null };
                      setCurrentNote(updatedNote);
                      invoke("edit_note", { note: updatedNote }).then(() => get_notes_metadata());
                    }}
                    className="hover:text-blue-400 transition-colors shrink-0"
                  >
                    All Notes
                  </button>
                  {getPath(currentNote.parent_id).map((folder) => (
                    <div key={folder.id} className="flex items-center gap-1.5 shrink-0 min-w-0">
                      <span className="text-slate-700">/</span>
                      <button 
                        onClick={() => {
                          const updatedNote = { ...currentNote, parent_id: folder.id };
                          setCurrentNote(updatedNote);
                          invoke("edit_note", { note: updatedNote }).then(() => get_notes_metadata());
                        }}
                        className="hover:text-blue-400 transition-colors truncate max-w-[120px]"
                      >
                        {folder.title}
                      </button>
                    </div>
                  ))}
                </div>

                {/* Move to Dropdown */}
                <div className="flex items-center gap-2">
                  <div className="relative group/move">
                    <select 
                      value={currentNote.parent_id || ""} 
                      onChange={(e) => {
                        const newParentId = e.target.value === "" ? null : e.target.value;
                        const updatedNote = { ...currentNote, parent_id: newParentId };
                        setCurrentNote(updatedNote);
                        invoke("edit_note", { note: updatedNote }).then(() => get_notes_metadata());
                      }}
                      className="appearance-none bg-slate-800 border border-slate-700 rounded-md pl-2 pr-6 py-1 text-[11px] font-medium text-slate-400 hover:text-slate-200 hover:border-slate-600 focus:outline-none transition-all cursor-pointer"
                    >
                      <option value="">Move to Root</option>
                      {notes.filter(n => n.is_folder && n.id !== currentNote.id).map(f => (
                        <option key={f.id} value={f.id}>Move to: {f.title}</option>
                      ))}
                    </select>
                    <div className="absolute right-1.5 top-1/2 -translate-y-1/2 pointer-events-none text-slate-600">
                      <svg className="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
                      </svg>
                    </div>
                  </div>
                </div>
              </div>

              <div className="flex items-center gap-3 overflow-hidden">
                <button
                  onClick={() => setSidebarOpen(true)}
                  className="lg:hidden shrink-0 p-2 -ml-2 text-slate-400 hover:text-white transition-colors"
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
                  placeholder={currentNote.is_folder ? "Folder title..." : "Note title..."}
                />
                <div className="ml-auto shrink-0 flex items-center gap-3">
                  {currentNote.deleted ? (
                    <button
                      onClick={() => restore_note(currentNote.id)}
                      className="text-xs font-medium text-emerald-400 bg-emerald-400/10 border border-emerald-400/20 hover:bg-emerald-400/20 px-2.5 py-1.5 rounded-md transition-colors flex items-center gap-1.5"
                    >
                      <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6" />
                      </svg>
                      Restore
                    </button>
                  ) : (
                    <button
                      onClick={() => setShowDeleteNoteConfirm(true, currentNote.id)}
                      className="text-xs font-medium text-red-400/80 bg-red-400/10 border border-red-400/20 hover:bg-red-400/20 px-2.5 py-1.5 rounded-md transition-colors flex items-center gap-1.5"
                    >
                      <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                      </svg>
                      Delete
                    </button>
                  )}
                  <div className="hidden sm:flex flex-col items-end text-[10px] text-slate-500 font-medium uppercase tracking-wider leading-none gap-1">
                    <span>Last Edit</span>
                    <span className="text-slate-400">{new Date(currentNote.updated_at).toLocaleString()}</span>
                  </div>
                </div>
              </div>
            </div>

            {/* Note Content */}
            <div className="flex-1 p-3 md:px-6 md:py-6 overflow-y-auto overflow-x-hidden">
              {!currentNote.is_folder ? (
                <textarea
                  onChange={(e) => edit_note(e.target.value)}
                  value={currentNote.content}
                  disabled={currentNote.deleted}
                  className="w-full h-full bg-transparent text-white resize-none border-none focus:outline-none placeholder-slate-600 text-sm md:text-base leading-relaxed disabled:opacity-60 disabled:cursor-not-allowed selection:bg-blue-500/30"
                  placeholder="Start writing..."
                />
              ) : (
                <div className="flex flex-col items-center justify-center h-full text-slate-500 opacity-40 select-none">
                  <div className="p-8 bg-slate-800/50 rounded-full mb-6">
                    <svg className="w-16 h-16" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
                    </svg>
                  </div>
                  <h3 className="text-xl font-bold text-slate-400 mb-2">Empty Folder</h3>
                  <p className="text-sm text-center max-w-xs">This folder has no notes yet. Use the <span className="text-blue-500 inline-flex items-center">+ <svg className="w-3 h-3 ml-1" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" /></svg></span> icons in the sidebar to add something here.</p>
                </div>
              )}
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
            <div className="flex-1 flex items-center justify-center text-slate-500 p-4 select-none">
              <div className="text-center opacity-40">
                <svg
                  className="w-12 h-12 md:w-16 md:h-16 mx-auto mb-4"
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
                <p className="text-base md:text-lg font-medium">Select a note to view</p>
                <p className="text-xs md:text-sm mt-1">Or create a new one to get started</p>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}