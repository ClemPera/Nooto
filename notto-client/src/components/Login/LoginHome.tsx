import { useState } from "react";
import Login from "./Login";
import CreateAccount from "./CreateAccount";
import { useGeneral } from "../../store/general";
import { invoke } from "@tauri-apps/api/core";
import { Workspace } from "../AccountMenu";

export default function LoginHome() {
  const [view, setView] = useState<"login" | "create">("login");
  const { setWorkspace, allWorkspaces } = useGeneral();

  async function handleLogBack(workspace: Workspace) {
    setWorkspace(workspace)
    await invoke("set_logged_workspace", { workspace_name: workspace.workspace_name });
  }

  return (
    <div className="flex items-center justify-center min-h-screen bg-linear-to-br from-slate-900 via-slate-800 to-slate-900 p-4">
      <div className="w-full max-w-md">
        <div className="text-center mb-6">
          <h1 className="text-3xl md:text-4xl font-bold text-white mb-2">Notto</h1>
          <p className="text-sm md:text-base text-slate-400">Secure, encrypted note-taking</p>
        </div>

        {allWorkspaces &&
          <>
            <p className="text-center text-white mb-1">Select your workspace: </p>
            <div className="flex gap-2 justify-center mb-3 text-white">
              {allWorkspaces
                .map(workspace => (
                  <button 
                    onClick={() => handleLogBack(workspace)}
                    className="py-2 px-3 md:px-4 rounded-md font-medium text-sm md:text-base transition-colors bg-slate-600 hover:bg-blue-600 text-white"
                  >
                    {workspace.workspace_name}
                  </button>
                ))
              }
            </div>
          </>
        }

        <div className="bg-slate-800 rounded-lg shadow-xl p-4 md:p-6 border border-slate-700">
          <div className="flex gap-2 mb-6">
            <button
              onClick={() => setView("login")}
              className={`flex-1 py-2 px-3 md:px-4 rounded-md font-medium text-sm md:text-base transition-colors ${view === "login"
                ? "bg-blue-600 text-white"
                : "bg-slate-700 text-slate-300 hover:bg-slate-600"
                }`}
            >
              Login
            </button>
            <button
              onClick={() => setView("create")}
              className={`flex-1 py-2 px-3 md:px-4 rounded-md font-medium text-sm md:text-base transition-colors ${view === "create"
                ? "bg-blue-600 text-white"
                : "bg-slate-700 text-slate-300 hover:bg-slate-600"
                }`}
            >
              Create Account
            </button>
          </div>

          {view === "login" ? <Login /> : <CreateAccount />}
        </div>

        <p className="text-center text-slate-500 text-xs md:text-sm mt-4">
          End-to-end encrypted • Zero-knowledge architecture
        </p>
      </div>
    </div>
  );
}
