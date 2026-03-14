import { create } from "zustand"

export type ConflictNoteVersion = {
  title: string
  content: string
  updated_at: number
}

export type ConflictPayload = {
  uuid: string
  local: ConflictNoteVersion
  server: ConflictNoteVersion
}

type ModalsStore = {
  showLogoutWorkspaceConfirm: boolean
  showDeleteNoteConfirm: boolean
  noteIdToDelete: string | null
  conflictPayload: ConflictPayload | null

  setShowLogoutWorkspaceConfirm: (show: boolean) => void
  setShowDeleteNoteConfirm: (show: boolean, noteId?: string) => void
  setConflictPayload: (payload: ConflictPayload | null) => void
}

export const useModals = create<ModalsStore>((set) => ({
  showLogoutWorkspaceConfirm: false,
  showDeleteNoteConfirm: false,
  noteIdToDelete: null,
  conflictPayload: null,

  setShowLogoutWorkspaceConfirm: (show) => {
    set(() => ({ showLogoutWorkspaceConfirm: show }))
  },
  setShowDeleteNoteConfirm: (show, noteId) => {
    set(() => ({ showDeleteNoteConfirm: show, noteIdToDelete: noteId ?? null }))
  },
  setConflictPayload: (payload) => {
    set(() => ({ conflictPayload: payload }))
  },
}))
