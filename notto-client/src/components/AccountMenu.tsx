import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { syncStatusEnum, useGeneral } from "../store/general";

export type Workspace = {
  id: number;
  workspace_name: string;
};

export default function AccountMenu() {
  const { workspace, setWorkspace, allWorkspaces, setShowLogoutConfirm, syncStatus } = useGeneral();
  const [showWorkspaceMenu, setShowWorkspaceMenu] = useState(false);

  async function switchAccount(workspace_name: string) {
    try {
      const workspace = await invoke("set_logged_workspace", { workspace_name }) as Workspace;
      setWorkspace(workspace);
      setShowWorkspaceMenu(false);
      
      window.location.reload();
    } catch (e) {
      console.error("Failed to switch account:", e);
    }
  }

  function addAccount() {
    setWorkspace(null);
    invoke("set_logged_workspace", { workspace_name: "" }).catch((e) => console.error(e));
  }
  return (
    <div className="border-t border-slate-700 bg-slate-800/50">
      {/* Sync Status */}
      <div className="px-2 md:px-3 py-2 flex items-center gap-2 text-xs md:text-sm">
        <div className={`w-2 h-2 rounded-full ${
          syncStatus === syncStatusEnum.synced ? "bg-green-500" :
          syncStatus === syncStatusEnum.syncing ? "bg-yellow-500 animate-pulse" :
          "bg-red-500"
        }`} />
        <span className="text-slate-400">
          {syncStatus === syncStatusEnum.synced ? "Synced" :
           syncStatus === syncStatusEnum.syncing ? "Syncing..." :
           syncStatus === syncStatusEnum.offline ? "Offline" :
           "Sync Error"}
        </span>
      </div>

      {/* Workspace Menu */}
      <div className="relative">
        <button
          onClick={() => setShowWorkspaceMenu(!showWorkspaceMenu)}
          className="w-full px-2 md:px-3 py-2 md:py-3 flex items-center justify-between hover:bg-slate-700/50 transition-colors text-left"
        >
          <div className="flex items-center gap-2 md:gap-3 min-w-0">
            <div className="w-7 h-7 md:w-8 md:h-8 rounded-full bg-blue-600 flex items-center justify-center text-white font-medium text-sm">
              {workspace?.workspace_name.charAt(0).toUpperCase() || "?"}
            </div>
            <div className="flex-1 min-w-0">
              <div className="text-xs md:text-sm font-medium text-white truncate">
                {workspace?.workspace_name || "No workspace"}
              </div>
              <div className="text-xs text-slate-400 hidden md:block">
                Click to switch account
              </div>
            </div>
          </div>
          <svg
            className={`w-4 h-4 text-slate-400 transition-transform shrink-0 ${showWorkspaceMenu ? "rotate-180" : ""}`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
          </svg>
        </button>

        {/* Dropdown Menu */}
        {showWorkspaceMenu && (
          <div className="absolute bottom-full left-0 right-0 mb-1 bg-slate-800 border border-slate-700 rounded-lg shadow-xl overflow-hidden">
            {/* Other Workspaces */}
            {allWorkspaces.filter(u => u.workspace_name !== workspace?.workspace_name).length > 0 && (
              <div className="py-1">
                <div className="px-2 md:px-3 py-2 text-xs font-medium text-slate-400 uppercase">
                  Switch Account
                </div>
                {allWorkspaces
                  .filter(u => u.workspace_name !== workspace?.workspace_name)
                  .map(workspace => (
                    <button
                      key={workspace.id}
                      onClick={() => switchAccount(workspace.workspace_name)}
                      className="w-full px-2 md:px-3 py-2 flex items-center gap-2 md:gap-3 hover:bg-slate-700 transition-colors text-left"
                    >
                      <div className="w-7 h-7 md:w-8 md:h-8 rounded-full bg-slate-600 flex items-center justify-center text-white text-xs md:text-sm font-medium">
                        {workspace.workspace_name.charAt(0).toUpperCase()}
                      </div>
                      <span className="text-xs md:text-sm text-white truncate">{workspace.workspace_name}</span>
                    </button>
                  )) }
              </div>
            ) }
            <div className="px-2 md:px-3 py-2 text-xs font-medium text-slate-400 uppercase">
                  <button
                    onClick={() => addAccount()}
                    className="w-full px-2 md:px-3 py-2 flex items-center gap-2 md:gap-3 hover:bg-slate-700 transition-colors text-left"
                  >
                  <span className="text-xs md:text-sm text-white truncate">Add another account</span>
                  </button>
            </div>


            {/* Actions */}
            <div className="border-t border-slate-700 py-1">
              <button
                onClick={() => {setShowLogoutConfirm(true); setShowWorkspaceMenu(false)}}
                className="w-full px-2 md:px-3 py-2 text-xs md:text-sm text-red-400 hover:bg-slate-700 transition-colors text-left flex items-center gap-2"
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                </svg>
                Logout
              </button>

            </div>
          </div>
        )}

      </div>
    </div>
  );
}
