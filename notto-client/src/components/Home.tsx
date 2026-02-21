import { useEffect, useState } from "react";
import { useGeneral } from "../store/general";
import { invoke } from "@tauri-apps/api/core";
import AccountMenu from "./AccountMenu";
import { trace } from "@tauri-apps/plugin-log";
import { listen } from "@tauri-apps/api/event";

type Note = {
  id: string;
  title: string;
  updated_at: Date;
};

type NoteContent = {
  id: string;
  title: string;
  content: string;
  updated_at: Date;
};

export default function Home() {
  const { workspace } = useGeneral();
  const [notes, setNotes] = useState<Note[] | null>(null);
  const [currentNote, setCurrentNote] = useState<NoteContent | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [sidebarOpen, setSidebarOpen] = useState(false);

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

  function get_notes_metadata() {
    if (workspace) {

      trace("getting notes metadata from: " + workspace?.id + " - " + workspace?.workspace_name)
      invoke("get_all_notes_metadata", { id_workspace: workspace?.id })
        .then((notes) => setNotes(notes as Note[]))
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
    };

    setCurrentNote(note);

    invoke("edit_note", { note }).catch((e) => console.error(e));
  }

  async function edit_note_title(title: string) {
    const note: NoteContent = {
      id: currentNote?.id!,
      title: title!,
      updated_at: currentNote?.updated_at!,
      content: currentNote?.content!,
    };

    setCurrentNote(note);

    await invoke("edit_note", { note }).catch((e) => console.error(e));

    get_notes_metadata();
  }

  const filteredNotes = notes?.filter((note) =>
    note.title.toLowerCase().includes(searchQuery.toLowerCase())
  );

  async function delete_note(id: string) {
    await invoke("delete_note", { id }).catch((e) => console.error(e));
  }

  return (
    <div className="flex h-screen pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] bg-slate-900">
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

          {/* Search */}
          <input
            type="text"
            placeholder="Search notes..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-white placeholder-slate-400 text-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
          />
        </div>

        {/* Notes List */}
        <div className="flex-1 overflow-y-auto">
          {filteredNotes && filteredNotes.length > 0 ? (
            <div className="p-2">
              {filteredNotes.map((note) => {
                const isActive = currentNote?.id === note.id;

                return (
                  <div
                    key={note.id}
                    className={`group w-full p-3 mb-1 rounded-lg text-left transition-colors flex items-center gap-2 ${
                      isActive
                        ? "bg-blue-600 text-white"
                        : "bg-slate-700/50 text-slate-200 hover:bg-slate-700"
                    }`}
                  >
                    <button
                      onClick={() => get_note(note.id)}
                      className="flex-1 text-left min-w-0"
                    >
                      <div className="font-medium truncate mb-1">{note.title}</div>
                      <div className="text-xs opacity-70">
                        {new Date(note.updated_at).toLocaleDateString()}
                      </div>
                    </button>

                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        delete_note(note.id);
                      }}
                      className={`shrink-0 p-1 rounded transition-colors ${
                        isActive
                          ? "text-blue-200 hover:text-red-300"
                          : "text-slate-500 hover:text-red-400"
                      }`}
                      title="Delete note"
                    >
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={2}
                          d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
                        />
                      </svg>
                    </button>
                  </div>
                );
              })}
            </div>
          ) : (
            <div className="flex items-center justify-center h-full text-slate-500 text-sm p-4 text-center">
              {searchQuery
                ? "No notes found"
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
            <div className="border-b border-slate-700 p-3 md:p-4 flex items-center gap-3">
              <button
                onClick={() => setSidebarOpen(true)}
                className="lg:hidden p-2 text-slate-400 hover:text-white transition-colors"
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
                className="flex-1 text-xl md:text-2xl font-bold bg-transparent text-white border-none focus:outline-none placeholder-slate-600"
                placeholder="Note title..."
              />
            </div>

            {/* Note Content */}
            <div className="flex-1 p-3 md:p-4 overflow-y-auto">
              <textarea
                onChange={(e) => edit_note(e.target.value)}
                value={currentNote.content}
                className="w-full h-full bg-transparent text-white resize-none border-none focus:outline-none placeholder-slate-600 text-sm md:text-base leading-relaxed"
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
