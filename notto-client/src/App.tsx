import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { useGeneral } from "./store/general";
import Home from "./components/Home";
import LoginHome from "./components/Login/LoginHome";
import { Workspace } from "./components/AccountMenu";
import LogoutConfirmModal from "./components/LogoutConfirmModal";

function App() {
  const { workspace, setWorkspace, setAllWorkspaces } = useGeneral();

  useEffect(() => {
    // Initialize the database on app start
    invoke("init").catch((e) => console.error(e));
    invoke("get_logged_workspace").then((u) => u as Workspace | null).then((u) => {
      if (u) {
        setWorkspace(u);
      };
    }).catch((e) => console.error(e));
  }, []);

  useEffect(() => {
    loadWorkspaces();
  }, [workspace]);

  async function loadWorkspaces() {
    try {
      const users = await invoke("get_workspaces") as Workspace[];
      setAllWorkspaces(users);

    } catch (e) {
      console.error("Failed to load users:", e);
    }
  }

  return (
    <div className="h-screen w-screen">
      {/* Modals */}
      <LogoutConfirmModal/>

      {workspace ? <Home /> : <LoginHome />}

    </div>
  );
}

export default App;
