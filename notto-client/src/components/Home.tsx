import { useEffect, useState } from "react";
import { useGeneral } from "../store/general";
import { invoke } from "@tauri-apps/api/core";
import AccountMenu from "./AccountMenu";
import { trace } from "@tauri-apps/plugin-log";
import { listen } from "@tauri-apps/api/event";

type Note = {
  id: number;
  title: string;
  updated_at: Date;
};

type NoteContent = {
  id: number;
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
    listen<Note[]>('new_note_metadata', (event) => {
      setNotes(event.payload)
    })
  }, [])

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

  async function get_note(id: number) {
    await invoke("get_note", { id: id })
      .then((note) => setCurrentNote(note as NoteContent))
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
              {filteredNotes.map((note) => (
                <button
                  key={note.id}
                  onClick={() => get_note(note.id)}
                  className={`w-full p-3 mb-1 rounded-lg text-left transition-colors ${currentNote?.id === note.id
                      ? "bg-blue-600 text-white"
                      : "bg-slate-700/50 text-slate-200 hover:bg-slate-700"
                    }`}
                >
                  <div className="font-medium truncate mb-1">{note.title}</div>
                  <div className="text-xs opacity-70">
                    {new Date(note.updated_at).toLocaleDateString()}
                  </div>
                </button>
              ))}
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
