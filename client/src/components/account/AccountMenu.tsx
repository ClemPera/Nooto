import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { syncStatusEnum, useGeneral } from "../../store/general";
import { useModals } from "../../store/modals";
import { listen } from "@tauri-apps/api/event";
import { trace } from "@tauri-apps/plugin-log";
import { handleCommandError, extractMessage } from "../../lib/errors";
import { Workspace } from "../../types";
import Icon from "../icons/Icon";
import AuthForm from "./AuthForm";
import WorkspaceMenu from "./WorkspaceMenu";

type AuthMode = "login" | "register";

const SYNC_LABEL: Record<syncStatusEnum, string> = {
  [syncStatusEnum.Synched]: "Synched",
  [syncStatusEnum.Syncing]: "Syncing...",
  [syncStatusEnum.Offline]: "Offline",
  [syncStatusEnum.NotConnected]: "Not connected",
  [syncStatusEnum.Error]: "Sync Error",
};

export default function AccountMenu() {
  const { workspace, setWorkspace, allWorkspaces, setSyncStatus, syncStatus } = useGeneral();
  const { setShowLogoutWorkspaceConfirm } = useModals();

  const [showMenu, setShowMenu] = useState(false);
  const [showWorkspaceMenu, setShowWorkspaceMenu] = useState(false);
  const [showAuthMenu, setShowAuthMenu] = useState(false);
  const [authMode, setAuthMode] = useState<AuthMode>("login");
  const [authLoading, setAuthLoading] = useState(false);
  const [authError, setAuthError] = useState("");
  const [versionNumber, setVersionNumber] = useState("");

  const menuRef = useRef<HTMLDivElement>(null);
  const workspaceMenuRef = useRef<HTMLDivElement>(null);
  const authMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    listen<syncStatusEnum>("sync-status", (event) => {
      trace("sync status: " + event.payload);
      setSyncStatus(event.payload);
    });
  }, []);

  useEffect(() => {
    invoke("get_version").then((v) => setVersionNumber(v as string));
  }, []);

  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      const target = event.target as Node;
      if (authMenuRef.current && !authMenuRef.current.contains(target)) {
        if (showAuthMenu) { setShowAuthMenu(false); setAuthError(""); return; }
      }
      if (workspaceMenuRef.current && !workspaceMenuRef.current.contains(target)) {
        if (showWorkspaceMenu) { setShowWorkspaceMenu(false); return; }
      }
      if (menuRef.current && !menuRef.current.contains(target)) {
        if (showMenu) { setShowMenu(false); setShowWorkspaceMenu(false); }
      }
    }
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [showMenu, showWorkspaceMenu, showAuthMenu]);

  async function handleAuthSubmit(username: string, password: string, instance: string) {
    setAuthLoading(true);
    setAuthError("");
    try {
      if (authMode === "register") {
        await invoke("sync_create_account", { username, password, instance });
      }
      await invoke("sync_login", { username, password, instance });
      setShowAuthMenu(false);
      setShowMenu(false);
      window.location.reload();
    } catch (e) {
      setAuthError(extractMessage(e, `Failed to ${authMode === "login" ? "login" : "create account"}`));
    } finally {
      setAuthLoading(false);
    }
  }

  async function switchAccount(workspace_name: string) {
    try {
      const ws = await invoke("set_logged_workspace", { workspace_name }) as Workspace;
      setWorkspace(ws);
      setShowMenu(false);
      setShowWorkspaceMenu(false);
      window.location.reload();
    } catch (e) {
      handleCommandError(e);
    }
  }

  async function addWorkspace() {
    const name = "workspace " + (allWorkspaces.length + 1);
    await invoke("create_workspace", { workspace_name: name }).catch(handleCommandError);
    await invoke("set_logged_workspace", { workspace_name: name }).catch(handleCommandError);
    const ws = await invoke("get_logged_workspace")
      .then((u) => u as Workspace | null)
      .catch(() => null);
    if (ws) setWorkspace(ws);
    window.location.reload();
  }

  const syncColor =
    syncStatus === syncStatusEnum.Synched ? "bg-green-500" :
    syncStatus === syncStatusEnum.Syncing ? "bg-yellow-500 animate-pulse" :
    "bg-red-500";

  return (
    <div className="border-t border-slate-700 bg-slate-800/50">
      {/* Sync status */}
      <div className="px-2 md:px-3 py-2 flex items-center gap-2 text-xs md:text-sm">
        <div className={`w-2 h-2 rounded-full ${syncColor}`} />
        <span className="text-slate-400">{SYNC_LABEL[syncStatus]}</span>
      </div>

      <div className="text-white text-xs px-2">version: {versionNumber}</div>

      {/* Account button */}
      <div className="relative" ref={menuRef}>
        <button
          onClick={() => setShowMenu(!showMenu)}
          className={`w-full px-2 md:px-3 py-2 md:py-3 flex items-center justify-between hover:bg-slate-700/50 transition-colors text-left ${showMenu ? "bg-slate-700/50" : ""}`}
        >
          <div className="flex items-center gap-2 md:gap-3 min-w-0">
            <div className="w-7 h-7 md:w-8 md:h-8 rounded-full bg-blue-600 flex items-center justify-center text-white font-medium text-sm">
              {workspace?.workspace_name.charAt(0).toUpperCase() || "?"}
            </div>
            <div className="flex-1 min-w-0">
              <div className="text-xs md:text-sm font-medium text-white truncate">
                {workspace?.workspace_name || "No workspace"}
              </div>
              <div className="text-xs text-slate-400 hidden md:block">Click to switch account</div>
            </div>
          </div>
          <Icon
            name={showMenu ? "chevronDown" : "chevronUp"}
            className="w-4 h-4 text-slate-400 shrink-0"
          />
        </button>

        {showMenu && (
          <div className="absolute bottom-full left-0 right-0 mb-1 bg-slate-800 border border-slate-700 rounded-lg shadow-xl overflow-y-scroll">
            {showWorkspaceMenu && (
              <div ref={workspaceMenuRef}>
                <WorkspaceMenu
                  workspaces={allWorkspaces}
                  current={workspace}
                  onSwitch={switchAccount}
                  onAdd={addWorkspace}
                  onLogoutWorkspace={() => {
                    setShowLogoutWorkspaceConfirm(true);
                    setShowMenu(false);
                    setShowWorkspaceMenu(false);
                  }}
                />
              </div>
            )}

            {showAuthMenu && (
              <div
                ref={authMenuRef}
                className="fixed bottom-16 left-0 right-0 mx-4 mb-1 bg-slate-800 border-2 border-slate-700 rounded-lg shadow-xl z-50 max-w-md"
              >
                {/* key={authMode} remounts form on mode toggle, resetting its local field state */}
                <AuthForm
                  key={authMode}
                  mode={authMode}
                  loading={authLoading}
                  error={authError}
                  onSubmit={handleAuthSubmit}
                  onToggleMode={() => {
                    setAuthMode(authMode === "login" ? "register" : "login");
                    setAuthError("");
                  }}
                />
              </div>
            )}

            <div className="border-t border-slate-700 py-1">
              <button
                onClick={() => setShowWorkspaceMenu(!showWorkspaceMenu)}
                className={`w-full px-2 md:px-3 py-2 text-xs md:text-sm text-white hover:bg-slate-700 transition-colors text-left flex items-center gap-2 ${showWorkspaceMenu ? "bg-slate-700" : ""}`}
              >
                <Icon name="list" />
                Manage workspaces
              </button>

              {syncStatus === syncStatusEnum.NotConnected ? (
                <button
                  onClick={() => { setShowAuthMenu(true); setAuthMode("login"); }}
                  className="w-full px-2 md:px-3 py-2 text-xs md:text-sm text-green-400 hover:bg-slate-700 transition-colors text-left flex items-center gap-2"
                >
                  <Icon name="user" />
                  Login/create account
                </button>
              ) : (
                <button
                  onClick={() => invoke("sync_logout").catch(handleCommandError)}
                  className="w-full px-2 md:px-3 py-2 text-xs md:text-sm text-red-400 hover:bg-slate-700 transition-colors text-left flex items-center gap-2"
                >
                  <Icon name="logoutIn" />
                  Logout
                </button>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
