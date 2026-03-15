import { create } from "zustand"
import { NoteContent } from "../components/Home"

type ModalsStore = {
  showLogoutWorkspaceConfirm: boolean
  showDeleteNoteConfirm: boolean
  noteIdToDelete: string | null
  conflictNote: NoteContent | null

  setShowLogoutWorkspaceConfirm: (show: boolean) => void
  setShowDeleteNoteConfirm: (show: boolean, noteId?: string) => void
  setConflictNote: (note: NoteContent | null) => void
}

export const useModals = create<ModalsStore>((set) => ({
  showLogoutWorkspaceConfirm: false,
  showDeleteNoteConfirm: false,
  noteIdToDelete: null,
  conflictNote: null,

  setShowLogoutWorkspaceConfirm: (show) => {
    set(() => ({ showLogoutWorkspaceConfirm: show }))
  },
  setShowDeleteNoteConfirm: (show, noteId) => {
    set(() => ({ showDeleteNoteConfirm: show, noteIdToDelete: noteId ?? null }))
  },
  setConflictNote: (note) => {
    set(() => ({ conflictNote: note }))
  },
}))
