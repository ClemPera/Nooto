import { handleCommandError } from "../../lib/errors";
import { invoke } from "@tauri-apps/api/core";
import { useGeneral } from "../../store/general";
import { Note } from "../../types";
import { useModals } from "../../store/modals";
import { trace } from "@tauri-apps/plugin-log";

export default function DeleteNoteConfirmModal() {
  const { workspace, notes, setNotes } = useGeneral();
  const { showDeleteNoteConfirm, noteIdToDelete, setShowDeleteNoteConfirm } = useModals();

  function get_notes_metadata() {
    if (!workspace) return;
    trace("getting notes metadata from: " + workspace.id + " - " + workspace.workspace_name);
    invoke("get_all_notes_metadata", { id_workspace: workspace.id })
      .then((fetched) => setNotes(fetched as Note[]))
      .catch(handleCommandError);
  }

  async function handleDelete() {
    if (!noteIdToDelete) return;
    const note = notes.find((n) => n.id === noteIdToDelete);
    if (!note) return;

    trace("deleting: " + noteIdToDelete + " is_folder: " + note.is_folder);

    if (note.is_folder) {
      // Move direct children up to the folder's parent before deleting
      const children = notes.filter((n) => n.parent_id === noteIdToDelete);
      for (const child of children) {
        const full: any = await invoke("get_note", { id: child.id });
        await invoke("edit_note", { note: { ...full, parent_id: note.parent_id } });
      }
    }

    await invoke("delete_note", { id: noteIdToDelete }).catch(handleCommandError);
    setShowDeleteNoteConfirm(false);
    get_notes_metadata();
  }

  const note = notes.find((n) => n.id === noteIdToDelete);
  const isFolder = note?.is_folder ?? false;

  return (
    <>
      {showDeleteNoteConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
          <div
            className="absolute inset-0 bg-slate-900/60 backdrop-blur-sm"
            onClick={() => setShowDeleteNoteConfirm(false)}
          />
          <div className="relative bg-slate-800 rounded-2xl shadow-2xl max-w-md w-full p-8 border border-slate-700">
            <div className="text-center mb-6">
              <div className="mx-auto w-16 h-16 bg-red-500/10 rounded-full flex items-center justify-center mb-4">
                <svg className="w-8 h-8 text-red-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
              </div>
              <h2 className="text-2xl font-bold text-white mb-2">Delete {isFolder ? "Folder" : "Note"}</h2>
              <p className="text-slate-300">
                {isFolder
                  ? "This folder will be deleted. All its contents will be moved to the parent folder."
                  : "This note will be moved to the trash."}
              </p>
            </div>

            <div className="flex flex-col gap-3">
              <button
                onClick={handleDelete}
                className="w-full px-6 py-3 bg-red-600 text-white font-semibold rounded-lg hover:bg-red-700 transition-all shadow-lg shadow-red-600/20"
              >
                Delete
              </button>
              <button
                onClick={() => setShowDeleteNoteConfirm(false)}
                className="w-full px-6 py-3 bg-slate-700 text-slate-200 font-semibold rounded-lg hover:bg-slate-600 transition-all"
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
