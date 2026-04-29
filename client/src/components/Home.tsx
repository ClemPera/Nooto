import { handleCommandError } from "../lib/errors";
import { useEffect, useState } from "react";
import { useGeneral } from "../store/general";
import { useModals } from "../store/modals";
import { invoke } from "@tauri-apps/api/core";
import { trace } from "@tauri-apps/plugin-log";
import { listen } from "@tauri-apps/api/event";
import { Note, NoteContent } from "../types";
import Sidebar from "./sidebar/Sidebar";
import NoteHeader from "./note/NoteHeader";
import FolderView from "./note/FolderView";
import NoteEditor from "./note/NoteEditor";
import Icon from "./icons/Icon";
import { TreeCallbacks } from "./sidebar/NoteTree";

export default function Home() {
  const { workspace, notes, setNotes } = useGeneral();
  const { setShowDeleteNoteConfirm } = useModals();
  const [currentNote, setCurrentNote] = useState<NoteContent | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [showDeleted, setShowDeleted] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listen<Note[]>("new_note_metadata", (event) => {
      const received = event.payload;
      setNotes(received);
      if (currentNote) {
        const updated = received.find((n) => n.id === currentNote.id);
        if (updated && updated.updated_at !== currentNote.updated_at) {
          get_note(updated.id);
        }
      }
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, [currentNote]);

  useEffect(() => {
    if (workspace) {
      get_notes_metadata();
      get_latest_note();
    }
  }, [workspace]);

  // Clear currentNote if it was removed or its deleted status no longer matches the current tab
  useEffect(() => {
    if (!currentNote) return;
    const inList = notes.find((n) => n.id === currentNote.id);
    if (!inList || inList.deleted !== showDeleted) setCurrentNote(null);
  }, [notes]);

  // When switching tabs, clear current note if it doesn't belong to the new tab
  useEffect(() => {
    if (!currentNote) return;
    const inList = notes.find((n) => n.id === currentNote.id);
    if (inList && inList.deleted !== showDeleted) setCurrentNote(null);
  }, [showDeleted]);

  /** Fetches all note metadata for the active workspace and updates the store. */
  function get_notes_metadata() {
    if (!workspace) return;
    trace("getting notes metadata from: " + workspace.id + " - " + workspace.workspace_name);
    invoke("get_all_notes_metadata", { id_workspace: workspace.id })
      .then((fetched) => setNotes(fetched as Note[]))
      .catch(handleCommandError);
  }

  /** Opens the last note the user had open when a workspace loads. */
  function get_latest_note() {
    if (currentNote) return;
    invoke("get_latest_note_id")
      .then((id) => { if (id) get_note(id as string); })
      .catch(handleCommandError);
  }

  /** Fetches the full decrypted content of a note and sets it as the current note. */
  async function get_note(id: string) {
    await invoke("get_note", { id })
      .then((note) => {
        setCurrentNote(note as NoteContent);
        trace("note received: " + (note as NoteContent).id);
      })
      .catch(handleCommandError);
  }

  /** Creates a new note under `parent_id` and immediately opens it. */
  async function create_note(parent_id: string | null = null) {
    await invoke("create_note", { title: "New Note", parent_id })
      .then((uuid) => get_note(uuid as string))
      .catch(handleCommandError);
    get_notes_metadata();
  }

  /** Creates a new folder under `parent_id` and refreshes the note tree. */
  async function create_folder(parent_id: string | null = null) {
    await invoke("create_folder", { title: "New Folder", parent_id })
      .then(() => get_notes_metadata())
      .catch(handleCommandError);
  }

  /** Toggles a folder's open/closed state in the sidebar. */
  async function toggle_folder(note: Note) {
    invoke("get_note", { id: note.id })
      .then((full: any) =>
        invoke("edit_note", { note: { ...full, folder_open: !note.folder_open } })
          .then(() => get_notes_metadata())
      )
      .catch(handleCommandError);
  }

  /** Saves updated note content; skips the call if content hasn't changed. */
  async function edit_note(content: string) {
    if (!currentNote || currentNote.content === content) return;
    const note: NoteContent = { ...currentNote, content };
    setCurrentNote(note);
    invoke("edit_note", { note }).catch(handleCommandError);
  }

  /** Updates the note title and refreshes the sidebar metadata. */
  async function edit_note_title(title: string) {
    const note: NoteContent = { ...currentNote!, title };
    setCurrentNote(note);
    await invoke("edit_note", { note }).catch(handleCommandError);
    get_notes_metadata();
  }

  /** Restores a soft-deleted note and clears the current selection. */
  async function restore_note(id: string) {
    await invoke("restore_note", { id }).catch(handleCommandError);
    setCurrentNote(null);
    get_notes_metadata();
  }

  /** Moves a note or folder to a new parent (drag-and-drop handler). */
  async function move_item(id: string, newParentId: string | null) {
    invoke("get_note", { id })
      .then((full: any) =>
        invoke("edit_note", { note: { ...full, parent_id: newParentId } })
          .then(() => get_notes_metadata())
      )
      .catch(handleCommandError);
  }

  const callbacks: TreeCallbacks = {
    onSelectNote: get_note,
    onToggleFolder: toggle_folder,
    onCreateNote: create_note,
    onCreateFolder: create_folder,
    onDelete: (id) => setShowDeleteNoteConfirm(true, id),
    onRestore: restore_note,
    onCloseSidebar: () => setSidebarOpen(false),
  };

  const deletedCount = notes.filter((n) => n.deleted && !n.is_folder).length;

  return (
    <div className="flex h-screen pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] bg-slate-900 overflow-hidden overscroll-none">
      {sidebarOpen && (
        <div
          className="fixed inset-0 bg-black/50 z-40 lg:hidden"
          onClick={() => setSidebarOpen(false)}
        />
      )}

      <Sidebar
        notes={notes}
        currentNoteId={currentNote?.id ?? null}
        sidebarOpen={sidebarOpen}
        showDeleted={showDeleted}
        deletedCount={deletedCount}
        onClose={() => setSidebarOpen(false)}
        onToggleDeleted={setShowDeleted}
        onMoveItem={move_item}
        onCreateNote={create_note}
        onCreateFolder={create_folder}
        callbacks={callbacks}
      />

      <div className="flex-1 flex flex-col bg-slate-900">
        {currentNote ? (
          <>
            <NoteHeader
              note={currentNote}
              onOpenSidebar={() => setSidebarOpen(true)}
              onEditTitle={edit_note_title}
              onDelete={() => setShowDeleteNoteConfirm(true, currentNote.id)}
              onRestore={() => restore_note(currentNote.id)}
            />
            <div className="flex-1 flex flex-col overflow-hidden">
              {currentNote.is_folder ? (
                <FolderView
                  folderId={currentNote.id}
                  notes={notes}
                  onSelect={get_note}
                />
              ) : (
                <NoteEditor
                  key={currentNote.id}
                  noteId={currentNote.id}
                  content={currentNote.content}
                  onChange={edit_note}
                  disabled={currentNote.deleted}
                />
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
                <Icon name="menu" className="w-5 h-5" />
              </button>
            </div>
            <div className="flex-1 flex items-center justify-center text-slate-500 p-4 select-none">
              <div className="text-center opacity-40">
                <Icon name="document" className="w-12 h-12 md:w-16 md:h-16 mx-auto mb-4" strokeWidth={1.5} />
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
