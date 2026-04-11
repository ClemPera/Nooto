/** A workspace as exposed to the frontend (no encryption material). */
export type Workspace = {
  id: number;
  workspace_name: string;
};

/** Decrypted note metadata (no content) used to populate the sidebar. */
export type Note = {
  id: string;
  title: string;
  parent_id: string | null;
  is_folder: boolean;
  folder_open: boolean;
  updated_at: Date;
  deleted: boolean;
};

/** Full decrypted note including content, returned by `get_note`. */
export type NoteContent = {
  id: string;
  title: string;
  parent_id: string | null;
  is_folder: boolean;
  folder_open: boolean;
  content: string;
  updated_at: Date;
  deleted: boolean;
};
