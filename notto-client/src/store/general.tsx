import { create } from "zustand"
import { Workspace } from "../components/AccountMenu"

export enum syncStatusEnum {
  synced = "synced",
  syncing = "sincing",
  error = "error",
  offline = "offline"
}

type Store = {
  workspace: Workspace | null
  allWorkspaces: Workspace[]
  showLogoutConfirm: boolean
  syncStatus: syncStatusEnum

  setWorkspace: (newWorkspace: Workspace | null) => void
  setAllWorkspaces: (newWorkspaces: Workspace[]) => void
  setShowLogoutConfirm: (confirm: boolean) => void
  setSyncStatus: (status: syncStatusEnum) => void
}

export const useGeneral = create<Store>(
  (set) => ({
    workspace: null,
    allWorkspaces: [],
    showLogoutConfirm: false,
    syncStatus: syncStatusEnum.offline,

    setWorkspace: (newWorkspace) => {
      set(() => ({ workspace: newWorkspace }))
    },
    setAllWorkspaces: (newWorkspaces) => {
      set(() => ({ allWorkspaces: newWorkspaces }))
    },
    setShowLogoutConfirm: (confirm) => {
      set(() => ({ showLogoutConfirm: confirm }))
    },
    setSyncStatus: (status) => {
      set(() => ({ syncStatus: status }))
    }
  })
)