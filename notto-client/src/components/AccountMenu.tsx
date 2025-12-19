import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useGeneral } from "../store/general";

export type User = {
  id: number;
  username: string;
};

export default function AccountMenu() {
  const { userId, setUserId } = useGeneral();
  const [currentUser, setCurrentUser] = useState<User | null>(null);
  const [allUsers, setAllUsers] = useState<User[]>([]);
  const [showUserMenu, setShowUserMenu] = useState(false);
  const [syncStatus] = useState<"synced" | "syncing" | "error">("synced");

  useEffect(() => {
    loadUsers();
  }, [userId]);

  async function loadUsers() {
    try {
      const users = await invoke("get_users") as User[];
      setAllUsers(users);

      // Find current user (TODO: in a real app, you'd store the current username properly)
      if (users.length > 0 && userId) {
        setCurrentUser(users[0]); // TODO: For now, just use first user
      }
    } catch (e) {
      console.error("Failed to load users:", e);
    }
  }

  async function switchAccount(username: string) {
    try {
      await invoke("set_logged_user", { username });
      setShowUserMenu(false);
      // Refresh the app - TODO: you might want to reload or update state here
      window.location.reload();
    } catch (e) {
      console.error("Failed to switch account:", e);
    }
  }

  async function createNewAccount() {
    //TODO
  }

  function handleLogout() {
    // Clear user session
    setUserId(null);
    invoke("set_logged_user", { username: "" }).catch((e) => console.error(e));

    // TODO: Also clear tokens and encrypted data from memory
  }

  return (
    <div className="border-t border-slate-700 bg-slate-800/50">
      {/* Sync Status */}
      <div className="px-2 md:px-3 py-2 flex items-center gap-2 text-xs md:text-sm">
        <div className={`w-2 h-2 rounded-full ${
          syncStatus === "synced" ? "bg-green-500" :
          syncStatus === "syncing" ? "bg-yellow-500 animate-pulse" :
          "bg-red-500"
        }`} />
        <span className="text-slate-400">
          {syncStatus === "synced" ? "Synced" :
           syncStatus === "syncing" ? "Syncing..." :
           "Sync Error"}
        </span>
      </div>

      {/* User Menu */}
      <div className="relative">
        <button
          onClick={() => setShowUserMenu(!showUserMenu)}
          className="w-full px-2 md:px-3 py-2 md:py-3 flex items-center justify-between hover:bg-slate-700/50 transition-colors text-left"
        >
          <div className="flex items-center gap-2 md:gap-3 min-w-0">
            <div className="w-7 h-7 md:w-8 md:h-8 rounded-full bg-blue-600 flex items-center justify-center text-white font-medium text-sm">
              {currentUser?.username.charAt(0).toUpperCase() || "?"}
            </div>
            <div className="flex-1 min-w-0">
              <div className="text-xs md:text-sm font-medium text-white truncate">
                {currentUser?.username || "No user"}
              </div>
              <div className="text-xs text-slate-400 hidden md:block">
                Click to switch account
              </div>
            </div>
          </div>
          <svg
            className={`w-4 h-4 text-slate-400 transition-transform shrink-0 ${showUserMenu ? "rotate-180" : ""}`}
            fill="none"
            stroke="currentColor"
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M5 15l7-7 7 7" />
          </svg>
        </button>

        {/* Dropdown Menu */}
        {showUserMenu && (
          <div className="absolute bottom-full left-0 right-0 mb-1 bg-slate-800 border border-slate-700 rounded-lg shadow-xl overflow-hidden">
            {/* Other Users */}
            {allUsers.filter(u => u.username !== currentUser?.username).length > 0 && (
              <div className="py-1">
                <div className="px-2 md:px-3 py-2 text-xs font-medium text-slate-400 uppercase">
                  Switch Account
                </div>
                {allUsers
                  .filter(u => u.username !== currentUser?.username)
                  .map(user => (
                    <button
                      key={user.id}
                      onClick={() => switchAccount(user.username)}
                      className="w-full px-2 md:px-3 py-2 flex items-center gap-2 md:gap-3 hover:bg-slate-700 transition-colors text-left"
                    >
                      <div className="w-7 h-7 md:w-8 md:h-8 rounded-full bg-slate-600 flex items-center justify-center text-white text-xs md:text-sm font-medium">
                        {user.username.charAt(0).toUpperCase()}
                      </div>
                      <span className="text-xs md:text-sm text-white truncate">{user.username}</span>
                    </button>
                  )) }
              </div>
            ) }
            <div className="px-2 md:px-3 py-2 text-xs font-medium text-slate-400 uppercase">
                  <button
                    onClick={() => createNewAccount()}
                    className="w-full px-2 md:px-3 py-2 flex items-center gap-2 md:gap-3 hover:bg-slate-700 transition-colors text-left"
                  >
                  <span className="text-xs md:text-sm text-white truncate">Add another account</span>
                  </button>
            </div>


            {/* Actions */}
            <div className="border-t border-slate-700 py-1">
              <button
                onClick={handleLogout}
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
