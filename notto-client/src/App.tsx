import { useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import { useGeneral } from "./store/general";
import Home from "./components/Home";
import LoginHome from "./components/Login/LoginHome";
import { User } from "./components/AccountMenu";

function App() {
  const { userId, setUserId } = useGeneral();

  useEffect(() => {
    // Initialize the database on app start
    invoke("init").catch((e) => console.error(e));
    invoke("get_logged_user").then((u) => u as User | null).then((u) => {
      if (u) {
        setUserId(u.id);
      };
    }).catch((e) => console.error(e));
  }, []);

  return (
    <div className="h-screen w-screen">
      {userId ? <Home /> : <LoginHome />}
    </div>
  );
}

export default App;
