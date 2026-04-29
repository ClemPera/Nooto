import { create } from "zustand";
import { NoteContent } from "../types";

type ModalsStore = {
  showLogoutWorkspaceConfirm: boolean;
  showDeleteNoteConfirm: boolean;
  noteIdToDelete: string | null;
  conflictNote: NoteContent | null;

  setShowLogoutWorkspaceConfirm: (show: boolean) => void;
  /** Opens or closes the delete confirmation modal. Pass `noteId` when opening. */
  setShowDeleteNoteConfirm: (show: boolean, noteId?: string) => void;
  /** Sets the note that triggered a sync conflict, or clears it when the modal is dismissed. */
  setConflictNote: (note: NoteContent | null) => void;
};

/** Store for modal visibility state and the data each modal requires. */
export const useModals = create<ModalsStore>((set) => ({
  showLogoutWorkspaceConfirm: false,
  showDeleteNoteConfirm: false,
  noteIdToDelete: null,
  conflictNote: null,

  setShowLogoutWorkspaceConfirm: (show) => set(() => ({ showLogoutWorkspaceConfirm: show })),
  setShowDeleteNoteConfirm: (show, noteId) =>
    set(() => ({ showDeleteNoteConfirm: show, noteIdToDelete: noteId ?? null })),
  setConflictNote: (note) => set(() => ({ conflictNote: note })),
}));
