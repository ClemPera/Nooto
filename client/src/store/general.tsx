import { create } from "zustand";
import { Note, Workspace } from "../types";

/** Mirrors the `SyncStatus` enum emitted by the Tauri sync service. */
export enum syncStatusEnum {
  Synched = "Synched",
  Syncing = "Syncing",
  Error = "Error",
  Offline = "Offline",
  NotConnected = "NotConnected",
  UpgradeRequired = "UpgradeRequired",
}

/** Payload emitted by the `version-mismatch` event. */
export type ProtocolMismatch = {
  code: string;
  client_outdated: boolean;
  client_protocol_version: number | null;
  server: {
    server_version: string;
    protocol_version: number;
    min_client_protocol_version: number;
  } | null;
};

const SYNC_LABEL: Record<syncStatusEnum, string> = {
  [syncStatusEnum.Synched]: "Synched",
  [syncStatusEnum.Syncing]: "Syncing...",
  [syncStatusEnum.Offline]: "Offline",
  [syncStatusEnum.NotConnected]: "Not connected",
  [syncStatusEnum.Error]: "Sync Error",
  [syncStatusEnum.UpgradeRequired]: "Update required",
};

export function syncStatusLabel(status: syncStatusEnum): string {
  return SYNC_LABEL[status];
}

type Store = {
  workspace: Workspace | null;
  allWorkspaces: Workspace[];
  notes: Note[];
  syncStatus: syncStatusEnum;
  versionMismatch: ProtocolMismatch | null;

  setWorkspace: (newWorkspace: Workspace | null) => void;
  setAllWorkspaces: (newWorkspaces: Workspace[]) => void;
  setNotes: (notes: Note[]) => void;
  setSyncStatus: (status: syncStatusEnum) => void;
  setVersionMismatch: (mismatch: ProtocolMismatch | null) => void;
};

/** Global store for workspace, note list, and sync status. */
export const useGeneral = create<Store>((set) => ({
  workspace: null,
  allWorkspaces: [],
  notes: [],
  syncStatus: syncStatusEnum.Offline,
  versionMismatch: null,

  setWorkspace: (newWorkspace) => set(() => ({ workspace: newWorkspace })),
  setAllWorkspaces: (newWorkspaces) => set(() => ({ allWorkspaces: newWorkspaces })),
  setNotes: (notes) => set(() => ({ notes })),
  setSyncStatus: (status) => set(() => ({ syncStatus: status })),
  setVersionMismatch: (mismatch) => set(() => ({ versionMismatch: mismatch })),
}));
