import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { useGeneral } from "./store/general";
import Home from "./components/Home";
import { Workspace } from "./components/AccountMenu";
import LogoutWorkspaceConfirmModal from "./components/LogoutWorkspaceConfirmModal";
import { info } from "@tauri-apps/plugin-log";

function App() {
  const { workspace, setWorkspace, setAllWorkspaces } = useGeneral();
  const hasInitialized = useRef(false);

  useEffect(() => {
    if (hasInitialized.current) return;
    hasInitialized.current = true;

    init();
  }, []);

  async function init() {
    await invoke("init").catch((e) => console.error(e));

    let backend_workspaces = await invoke("get_workspaces")
      .then((u) => u as Workspace[])
      .catch((e) => {
        console.error(e);
        return [];
      });

    if (backend_workspaces.length <= 0) {
      // Create default workspace
      await invoke("create_workspace", { workspace_name: "workspace 1" })
        .catch((e) => console.error(e));
    }

    backend_workspaces = await invoke("get_workspaces")
      .then((u) => u as Workspace[])
      .catch((e) => {
        console.error(e);
        return [];
      });

    setAllWorkspaces(backend_workspaces);

    // Get and set the logged workspace
    const loggedWorkspace = await invoke("get_logged_workspace")
      .then((u) => u as Workspace | null)
      .catch((e) => console.error(e));

    if (loggedWorkspace) {
      setWorkspace(loggedWorkspace);
    }
  }

  return (
    <div className="h-screen w-screen">
      {/* Modals */}
      <LogoutWorkspaceConfirmModal />

      {workspace ? <Home /> : <div className="text-center bg-slate-800">Creating workspace...</div>}

    </div>
  );
}

export default App;
