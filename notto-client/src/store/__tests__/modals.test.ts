import { describe, it, expect, beforeEach } from "vitest";
import { useModals } from "../modals";
import { NoteContent } from "../../types";

const mockNote: NoteContent = {
  id: "note-1",
  title: "Test Note",
  parent_id: null,
  is_folder: false,
  folder_open: false,
  content: "hello",
  updated_at: new Date(),
  deleted: false,
};

beforeEach(() => {
  useModals.setState({
    showLogoutWorkspaceConfirm: false,
    showDeleteNoteConfirm: false,
    noteIdToDelete: null,
    conflictNote: null,
  });
});

describe("useModals", () => {
  it("starts with all modals closed", () => {
    const state = useModals.getState();
    expect(state.showLogoutWorkspaceConfirm).toBe(false);
    expect(state.showDeleteNoteConfirm).toBe(false);
    expect(state.noteIdToDelete).toBeNull();
    expect(state.conflictNote).toBeNull();
  });

  it("shows and hides logout workspace confirm", () => {
    useModals.getState().setShowLogoutWorkspaceConfirm(true);
    expect(useModals.getState().showLogoutWorkspaceConfirm).toBe(true);

    useModals.getState().setShowLogoutWorkspaceConfirm(false);
    expect(useModals.getState().showLogoutWorkspaceConfirm).toBe(false);
  });

  it("shows delete note confirm with a note id", () => {
    useModals.getState().setShowDeleteNoteConfirm(true, "note-42");
    const state = useModals.getState();
    expect(state.showDeleteNoteConfirm).toBe(true);
    expect(state.noteIdToDelete).toBe("note-42");
  });

  it("clears note id when hiding delete confirm without id", () => {
    useModals.getState().setShowDeleteNoteConfirm(true, "note-42");
    useModals.getState().setShowDeleteNoteConfirm(false);
    const state = useModals.getState();
    expect(state.showDeleteNoteConfirm).toBe(false);
    expect(state.noteIdToDelete).toBeNull();
  });

  it("sets and clears conflict note", () => {
    useModals.getState().setConflictNote(mockNote);
    expect(useModals.getState().conflictNote).toEqual(mockNote);

    useModals.getState().setConflictNote(null);
    expect(useModals.getState().conflictNote).toBeNull();
  });
});
