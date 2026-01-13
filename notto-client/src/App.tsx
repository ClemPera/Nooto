import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { useGeneral } from "./store/general";
import Home from "./components/Home";
import LoginHome from "./components/Login/LoginHome";
import { Workspace } from "./components/AccountMenu";
import LogoutConfirmModal from "./components/LogoutConfirmModal";
import { info } from "@tauri-apps/plugin-log";

function App() {
  const { workspace, setWorkspace, setAllWorkspaces } = useGeneral();

  useEffect(() => {
    init();
  }, []);

  useEffect(() => {
    loadWorkspaces();
  }, [workspace]);

  async function init() {
    await invoke("init").catch((e) => console.error(e));

    let loggedWorkspace = await invoke("get_logged_workspace")
      .then((u) => u as Workspace | null)
      .catch((e) => console.error(e));

    if (loggedWorkspace) {
      setWorkspace(loggedWorkspace);
    }
  }

  async function loadWorkspaces() {
    try {
      const backend_workspaces = await invoke("get_workspaces") as Workspace[];
      setAllWorkspaces(backend_workspaces);

      if(backend_workspaces.length <= 0) {
        await invoke("create_workspace", { workspace_name: "workspace 1" }).catch((e) => console.error(e));
        
        await invoke("get_logged_workspace").then((u) => u as Workspace | null).then((u) => {
          if (u) {
            setWorkspace(u);
          };
        }).catch((e) => console.error(e));
      }

      //This doesn't work - removing for now, lets see if it cause any issue
      // }else if(backend_workspaces.length >= 1 && workspace == null) {
      //   await invoke("set_logged_workspace", { workspace_name: backend_workspaces[0].workspace_name });
      //   info("load and set workspace:" + backend_workspaces[0].workspace_name);

      //   await invoke("get_logged_workspace").then((u) => u as Workspace | null).then((u) => {
      //     if (u) {
      //       setWorkspace(u);
      //     };
      //   }).catch((e) => console.error(e));
      // }
    } catch (e) {
      console.error("Failed to load backend_workspaces:", e);
    }
  }

  return (
    <div className="h-screen w-screen">
      {/* Modals */}
      <LogoutConfirmModal />

      {workspace ? <Home /> : <div className="text-center bg-slate-800">Creating workspace...</div>}

    </div>
  );
}

export default App;
