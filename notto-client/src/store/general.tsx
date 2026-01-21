import { create } from "zustand"
import { Workspace } from "../components/AccountMenu"

export enum syncStatusEnum {
  Synched = "Synched",
  Syncing = "Syncing", 
  Error = "Error", 
  Offline = "Offline",
  NotConnected = "NotConnected"
}

type Store = {
  workspace: Workspace | null
  allWorkspaces: Workspace[]
  showLogoutWorkspaceConfirm: boolean
  syncStatus: syncStatusEnum

  setWorkspace: (newWorkspace: Workspace | null) => void
  setAllWorkspaces: (newWorkspaces: Workspace[]) => void
  setShowLogoutWorkspaceConfirm: (confirm: boolean) => void
  setSyncStatus: (status: syncStatusEnum) => void
}

export const useGeneral = create<Store>(
  (set) => ({
    workspace: null,
    allWorkspaces: [],
    showLogoutWorkspaceConfirm: false,
    syncStatus: syncStatusEnum.Offline,

    setWorkspace: (newWorkspace) => {
      set(() => ({ workspace: newWorkspace }))
    },
    setAllWorkspaces: (newWorkspaces) => {
      set(() => ({ allWorkspaces: newWorkspaces }))
    },
    setShowLogoutWorkspaceConfirm: (confirm) => {
      set(() => ({ showLogoutWorkspaceConfirm: confirm }))
    },
    setSyncStatus: (status) => {
      set(() => ({ syncStatus: status }))
    }
  })
)