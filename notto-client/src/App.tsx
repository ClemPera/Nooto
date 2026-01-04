import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { useGeneral } from "./store/general";
import Home from "./components/Home";
import LoginHome from "./components/Login/LoginHome";
import { User } from "./components/AccountMenu";
import LogoutConfirmModal from "./components/LogoutConfirmModal";

function App() {
  const { user, setUser, allUsers, setAllUsers } = useGeneral();

  useEffect(() => {
    // Initialize the database on app start
    invoke("init").catch((e) => console.error(e));
    invoke("get_logged_workspace").then((u) => u as User | null).then((u) => {
      if (u) {
        setUser(u);
      };
    }).catch((e) => console.error(e));
  }, []);

  useEffect(() => {
    loadUsers();
  }, [user]);

  async function loadUsers() {
    try {
      const users = await invoke("get_workspaces") as User[];
      setAllUsers(users);

    } catch (e) {
      console.error("Failed to load users:", e);
    }
  }

  return (
    <div className="h-screen w-screen">
      {/* Modals */}
      <LogoutConfirmModal/>

      {user ? <Home /> : <LoginHome />}

    </div>
  );
}

export default App;
