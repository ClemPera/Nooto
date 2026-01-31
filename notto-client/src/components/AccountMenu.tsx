import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { syncStatusEnum, useGeneral } from "../store/general";
import { listen } from "@tauri-apps/api/event";
import { trace } from "@tauri-apps/plugin-log";

export type Workspace = {
  id: number;
  workspace_name: string;
};

type AuthMode = "login" | "register";

export default function AccountMenu() {
  const { workspace, setWorkspace, allWorkspaces, setShowLogoutWorkspaceConfirm, setSyncStatus, syncStatus } = useGeneral();
  const [showAccountMenu, setShowAccountMenu] = useState(false);
  const [showWorkspaceMenu, setShowWorkspaceMenu] = useState(false);
  const [showAuthMenu, setShowAuthMenu] = useState(false);
  const [authMode, setAuthMode] = useState<AuthMode>("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [instance, setInstance] = useState("http://localhost:3000");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [authLoading, setAuthLoading] = useState(false);
  const [authError, setAuthError] = useState("");

  const accountMenuRef = useRef<HTMLDivElement>(null);
  const workspaceMenuRef = useRef<HTMLDivElement>(null);
  const authMenuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    listen<syncStatusEnum>('sync-status', (event) => {
      trace("sync status: " + event.payload)
      setSyncStatus(event.payload)
    })
  }, [])

  // Handle click outside
  useEffect(() => {
    function handleClickOutside(event: MouseEvent) {
      // Check if click is outside auth menu
      if (authMenuRef.current && !authMenuRef.current.contains(event.target as Node)) {
        if (showAuthMenu) {
          setShowAuthMenu(false);
          resetAuthForm();
          return;
        }
      }

      // Check if click is outside workspace menu
      if (workspaceMenuRef.current && !workspaceMenuRef.current.contains(event.target as Node)) {
        if (showWorkspaceMenu) {
          setShowWorkspaceMenu(false);
          return;
        }
      }

      // Check if click is outside account menu
      if (accountMenuRef.current && !accountMenuRef.current.contains(event.target as Node)) {
        if (showAccountMenu) {
          setShowAccountMenu(false);
          setShowWorkspaceMenu(false);
        }
      }
    }

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [showAccountMenu, showWorkspaceMenu, showAuthMenu]);

  function resetAuthForm() {
    setUsername("");
    setPassword("");
    setConfirmPassword("");
    setAuthError("");
    setShowAdvanced(false);
  }

  function toggleAuthMode() {
    setAuthMode(authMode === "login" ? "register" : "login");
    resetAuthForm();
  }

  async function handleAuthSubmit() {
    setAuthError("");

    if (!username || !password) {
      setAuthError("Please fill in all fields");
      return;
    }

    if (authMode === "register" && password !== confirmPassword) {
      setAuthError("Passwords do not match");
      return;
    }

    setAuthLoading(true);

    try {
      if (authMode === "login") {
        await invoke("sync_login", { username, password, instance: instance });
      } else {
        await invoke("sync_create_account", { username ,password, instance: instance });
        await invoke("sync_login", { username, password, instance: instance });
      }
      
      setShowAuthMenu(false);
      setShowAccountMenu(false);
      resetAuthForm();
      window.location.reload();
    } catch (e) {
      setAuthError(e as string || `Failed to ${authMode === "login" ? "login" : "create account"}`);
    } finally {
      setAuthLoading(false);
    }
  }

  async function handleServerLogout() {
    await invoke("sync_logout").catch((e) => console.error(e));
  }

  async function switchAccount(workspace_name: string) {
    try {
      const workspace = await invoke("set_logged_workspace", { workspace_name }) as Workspace;
      setWorkspace(workspace);
      setShowAccountMenu(false);
      setShowWorkspaceMenu(false);

      window.location.reload();
    } catch (e) {
      console.error("Failed to switch account:", e);
    }
  }

  async function addWorkspace() {
    console.log("all workspaces are:", allWorkspaces);
    console.log("adding workspace:", allWorkspaces.length + 1)

    //TODO
    let newName = "workspace " + (allWorkspaces.length + 1)

    await invoke("create_workspace", { workspace_name: newName }).catch((e) => console.error(e));
    await invoke("set_logged_workspace", { workspace_name: newName });
    await invoke("get_logged_workspace").then((u) => u as Workspace | null).then((u) => {
      if (u) {
        setWorkspace(u);
      };
    }).catch((e) => console.error(e));

    window.location.reload();
  }

  return (
    <div className="border-t border-slate-700 bg-slate-800/50">
      {/* Sync Status */}
      <div className="px-2 md:px-3 py-2 flex items-center gap-2 text-xs md:text-sm">
        <div className={`w-2 h-2 rounded-full ${syncStatus === syncStatusEnum.Synched ? "bg-green-500" :
          syncStatus === syncStatusEnum.Syncing ? "bg-yellow-500 animate-pulse" :
            "bg-red-500"
          }`} />
        <span className="text-slate-400">
          {syncStatus === syncStatusEnum.Synched ? "Synched" :
            syncStatus === syncStatusEnum.Syncing ? "Syncing..." :
              syncStatus === syncStatusEnum.Offline ? "Offline" :
                syncStatus === syncStatusEnum.NotConnected ? "Not connected" :
                  "Sync Error"}
        </span>
      </div>

      {/* Workspace Menu */}
      <div className="relative" ref={accountMenuRef}>
        <button
          onClick={() => setShowAccountMenu(!showAccountMenu)}
          className={`w-full px-2 md:px-3 py-2 md:py-3 flex items-center justify-between hover:bg-slate-700/50 transition-colors text-left ${showAccountMenu ? "bg-slate-700/50" : ""}`}
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
            className={`w-4 h-4 text-slate-400 transition-transform shrink-0 ${showAccountMenu ? "rotate-180" : ""}`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
          </svg>
        </button>

        {/* Dropdown Menu */}
        {showAccountMenu && (
          <div className="absolute bottom-full left-0 right-0 mb-1 bg-slate-800 border border-slate-700 rounded-lg shadow-xl overflow-y-scroll">
            {showWorkspaceMenu && (
              <div ref={workspaceMenuRef} className="fixed bottom-16 left-0 right-0 mx-4 mb-1 bg-slate-800 border-2 border-slate-700 rounded-lg shadow-xl overflow-y-scroll max-h-96 z-50">
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
                      ))}
                  </div>
                )}
                <div className="py-1">
                  <button
                    onClick={() => addWorkspace()}
                    className="w-full px-2 md:px-3 py-2 flex items-center gap-2 md:gap-3 hover:bg-slate-700 transition-colors text-left"
                  >
                    <span className="text-xs md:text-sm text-white truncate">Add another workspace</span>
                  </button>
                </div>

                {/* Workspace Actions */}
                <div className="border-t border-slate-700 py-1">
                  <button
                    onClick={() => { setShowLogoutWorkspaceConfirm(true); setShowAccountMenu(false); setShowWorkspaceMenu(false); }}
                    className="w-full px-2 md:px-3 py-2 text-xs md:text-sm text-red-400 hover:bg-slate-700 transition-colors text-left flex items-center gap-2"
                  >
                    <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1" />
                    </svg>
                    Logout from workspace
                  </button>
                </div>
              </div>
            )}

            {/* Auth Menu Modal */}
            {showAuthMenu && (
              <div ref={authMenuRef} className="fixed bottom-16 left-0 right-0 mx-4 mb-1 bg-slate-800 border-2 border-slate-700 rounded-lg shadow-xl z-50 max-w-md">
                <div className="p-4">
                  {/* Header */}
                  <div className="mb-4">
                    <h2 className="text-lg font-semibold text-white mb-1">
                      {authMode === "login" ? "Login to your account" : "Create a new account"}
                    </h2>
                    <p className="text-xs text-slate-400">
                      {authMode === "login" 
                        ? "Enter your credentials to sync your workspace" 
                        : "Sign up to sync your data across devices"}
                    </p>
                  </div>

                  {/* Error Message */}
                  {authError && (
                    <div className="mb-4 p-3 bg-red-500/10 border border-red-500/20 rounded-lg">
                      <p className="text-xs text-red-400">{authError}</p>
                    </div>
                  )}

                  {/* Form Fields */}
                  <div className="space-y-3">
                    <div>
                      <label className="block text-xs font-medium text-slate-300 mb-1">
                        Username
                      </label>
                      <input
                        type="text"
                        value={username}
                        onChange={(e) => setUsername(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && handleAuthSubmit()}
                        className="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                        placeholder="johndoe"
                        disabled={authLoading}
                      />
                    </div>

                    <div>
                      <label className="block text-xs font-medium text-slate-300 mb-1">
                        Password
                      </label>
                      <input
                        type="password"
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        onKeyDown={(e) => e.key === "Enter" && handleAuthSubmit()}
                        className="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                        placeholder="••••••••"
                        disabled={authLoading}
                      />
                    </div>

                    {authMode === "register" && (
                      <div>
                        <label className="block text-xs font-medium text-slate-300 mb-1">
                          Confirm Password
                        </label>
                        <input
                          type="password"
                          value={confirmPassword}
                          onChange={(e) => setConfirmPassword(e.target.value)}
                          onKeyDown={(e) => e.key === "Enter" && handleAuthSubmit()}
                          className="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                          placeholder="••••••••"
                          disabled={authLoading}
                        />
                      </div>
                    )}

                    {/* Advanced Settings */}
                    <div className="pt-2">
                      <button
                        onClick={() => setShowAdvanced(!showAdvanced)}
                        className="flex items-center gap-2 text-xs text-slate-400 hover:text-slate-300 transition-colors"
                      >
                        <svg
                          className={`w-3 h-3 transition-transform ${showAdvanced ? "rotate-90" : ""}`}
                          fill="none"
                          stroke="currentColor"
                          viewBox="0 0 24 24"
                        >
                          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
                        </svg>
                        Advanced settings
                      </button>
                      
                      {showAdvanced && (
                        <div className="mt-3">
                          <label className="block text-xs font-medium text-slate-300 mb-1">
                            Server Address
                          </label>
                          <input
                            type="text"
                            value={instance}
                            onChange={(e) => setInstance(e.target.value)}
                            className="w-full px-3 py-2 bg-slate-700 border border-slate-600 rounded-lg text-sm text-white placeholder-slate-400 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                            placeholder="http://localhost:3000"
                            disabled={authLoading}
                          />
                        </div>
                      )}
                    </div>

                    <button
                      onClick={handleAuthSubmit}
                      disabled={authLoading}
                      className="w-full px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-blue-600/50 disabled:cursor-not-allowed text-white text-sm font-medium rounded-lg transition-colors"
                    >
                      {authLoading 
                        ? (authMode === "login" ? "Logging in..." : "Creating account...") 
                        : (authMode === "login" ? "Login" : "Create Account")}
                    </button>
                  </div>

                  {/* Toggle Auth Mode */}
                  <div className="mt-4 pt-4 border-t border-slate-700">
                    <button
                      onClick={toggleAuthMode}
                      className="w-full text-xs text-slate-400 hover:text-slate-300 transition-colors"
                    >
                      {authMode === "login" 
                        ? "Don't have an account? Create one" 
                        : "Already have an account? Login"}
                    </button>
                  </div>
                </div>
              </div>
            )}

            <div className="border-t border-slate-700 py-1">
              <button
                onClick={() => setShowWorkspaceMenu(!showWorkspaceMenu)}
                className={`w-full px-2 md:px-3 py-2 text-xs md:text-sm text-white hover:bg-slate-700 transition-colors text-left flex items-center gap-2 ${showWorkspaceMenu ? "bg-slate-700" : ""}`}
              >
                <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M8 7h12M8 12h12M8 17h12M3 7h.01M3 12h.01M3 17h.01" />
                </svg>
                Manage workspaces
              </button>

              {/* Server Actions */}
              {syncStatus == syncStatusEnum.NotConnected ?
                <button
                  onClick={() => { setShowAuthMenu(true); setAuthMode("login"); }}
                  className="w-full px-2 md:px-3 py-2 text-xs md:text-sm text-green-400 hover:bg-slate-700 transition-colors text-left flex items-center gap-2"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                  </svg>
                  Login/create account
                </button>
                :
                <button
                  onClick={handleServerLogout}
                  className="w-full px-2 md:px-3 py-2 text-xs md:text-sm text-red-400 hover:bg-slate-700 transition-colors text-left flex items-center gap-2"
                >
                  <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M7 16l-4-4m0 0l4-4m-4 4h14M11 20v1a3 3 0 003 3h4a3 3 0 003-3V7a3 3 0 00-3-3h-4a3 3 0 00-3 3v1" />
                  </svg>
                  Logout
                </button>
              }
            </div>
          </div>
        )}

      </div>
    </div>
  );
}