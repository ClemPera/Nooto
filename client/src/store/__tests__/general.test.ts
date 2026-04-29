import { describe, it, expect, beforeEach } from "vitest";
import { useGeneral, syncStatusEnum } from "../general";
import { Note, Workspace } from "../../types";

const mockWorkspace: Workspace = { id: 1, workspace_name: "test-ws" };

const mockNote: Note = {
  id: "note-1",
  title: "Hello",
  parent_id: null,
  is_folder: false,
  folder_open: false,
  updated_at: new Date(),
  deleted: false,
};

beforeEach(() => {
  useGeneral.setState({
    workspace: null,
    allWorkspaces: [],
    notes: [],
    syncStatus: syncStatusEnum.Offline,
  });
});

describe("useGeneral", () => {
  it("starts with null workspace and empty state", () => {
    const state = useGeneral.getState();
    expect(state.workspace).toBeNull();
    expect(state.allWorkspaces).toEqual([]);
    expect(state.notes).toEqual([]);
    expect(state.syncStatus).toBe(syncStatusEnum.Offline);
  });

  it("sets and clears workspace", () => {
    useGeneral.getState().setWorkspace(mockWorkspace);
    expect(useGeneral.getState().workspace).toEqual(mockWorkspace);

    useGeneral.getState().setWorkspace(null);
    expect(useGeneral.getState().workspace).toBeNull();
  });

  it("sets all workspaces", () => {
    const ws2: Workspace = { id: 2, workspace_name: "other" };
    useGeneral.getState().setAllWorkspaces([mockWorkspace, ws2]);
    expect(useGeneral.getState().allWorkspaces).toHaveLength(2);
  });

  it("sets notes", () => {
    useGeneral.getState().setNotes([mockNote]);
    expect(useGeneral.getState().notes).toHaveLength(1);
    expect(useGeneral.getState().notes[0].id).toBe("note-1");
  });

  it("replaces notes on subsequent setNotes calls", () => {
    useGeneral.getState().setNotes([mockNote]);
    const note2: Note = { ...mockNote, id: "note-2", title: "Second" };
    useGeneral.getState().setNotes([note2]);
    expect(useGeneral.getState().notes).toHaveLength(1);
    expect(useGeneral.getState().notes[0].id).toBe("note-2");
  });

  it("updates sync status", () => {
    useGeneral.getState().setSyncStatus(syncStatusEnum.Syncing);
    expect(useGeneral.getState().syncStatus).toBe(syncStatusEnum.Syncing);

    useGeneral.getState().setSyncStatus(syncStatusEnum.Synched);
    expect(useGeneral.getState().syncStatus).toBe(syncStatusEnum.Synched);

    useGeneral.getState().setSyncStatus(syncStatusEnum.Error);
    expect(useGeneral.getState().syncStatus).toBe(syncStatusEnum.Error);
  });
});
