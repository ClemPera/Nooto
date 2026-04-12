import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { useGeneral } from "./store/general";
import { Workspace } from "./types";
import Home from "./components/Home";
import LogoutWorkspaceConfirmModal from "./components/modals/LogoutWorkspaceConfirmModal";
import DeleteNoteConfirmModal from "./components/modals/DeleteNoteConfirmModal";
import ConflictModal from "./components/modals/ConflictModal";
import Toaster from "./components/Toaster";
import { handleCommandError } from "./lib/errors";

/** Root component — bootstraps the workspace on mount and renders the modal/toast layer. */
function App() {
  const { setWorkspace, setAllWorkspaces } = useGeneral();
  const hasInitialized = useRef(false);

  useEffect(() => {
    if (hasInitialized.current) return;
    hasInitialized.current = true;
    init();
  }, []);

  /** Loads the last active workspace on startup, creating a default one if none exist. */
  async function init() {
    await invoke("init").catch(handleCommandError);

    let workspaces = await invoke("get_workspaces")
      .then((u) => u as Workspace[])
      .catch((e) => { handleCommandError(e); return []; });

    if (workspaces.length === 0) {
      await invoke("create_workspace", { workspace_name: "workspace 1" }).catch(handleCommandError);
      await invoke("set_logged_workspace", { workspace_name: "workspace 1" }).catch(handleCommandError);
      workspaces = await invoke("get_workspaces")
        .then((u) => u as Workspace[])
        .catch((e) => { handleCommandError(e); return []; });
    }

    setAllWorkspaces(workspaces);

    const loggedWorkspace = await invoke("get_logged_workspace")
      .then((u) => u as Workspace | null)
      .catch((e) => { handleCommandError(e); return null; });

    if (loggedWorkspace) setWorkspace(loggedWorkspace);
  }

  const { workspace } = useGeneral();

  return (
    <div className="h-screen w-screen">
      <LogoutWorkspaceConfirmModal />
      <DeleteNoteConfirmModal />
      <ConflictModal />
      <Toaster />

      {!workspace && (
        <div className="flex grow place-items-center place-content-center text-2xl text-center text-white bg-slate-800 min-h-screen pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)] backdrop-blur-sm">
          Creating workspace...
        </div>
      )}
      <Home />
    </div>
  );
}

export default App;
