import { useEffect, useState } from "react";
import { useGeneral } from "../store/general";
import { invoke } from "@tauri-apps/api/core";
import { info } from "@tauri-apps/plugin-log";

enum SyncStatus {
  Ok = "Ok",
  Syncing = "Synching",
  Error = "Error",
}

export default function Sync() {
  const { userId, setUserId } = useGeneral();
  const [logged, setLogged] = useState<boolean>(false);
  const [syncStatus, setSyncStatus] = useState<SyncStatus>(SyncStatus.Syncing);

  async function create_account() {
    await invoke("sync_create_account", { username: "test_account", password: "password" })
      .catch((e) => console.error(e));
  }

  async function login() {
    await invoke("sync_login", { username: "test_account", password: "password" })
      .then((loggedStatus) => {
        setLogged(() => loggedStatus as boolean);
        console.log("user has been logged: ", loggedStatus as boolean);
        console.log("logged: ", logged)
      })
      .catch((e) => console.error(e));
  }

  useEffect(() => {
    let interval = setInterval(async () => {
      console.log("looping:", logged);
      
    },1000);

    return () => clearInterval(interval);
  }, [logged]);


  return (
    <div>
      <h3 className="text-xl">Server actions</h3>
      <button className="h-10 w-min p-2 bg-yellow-600 cursor-pointer" onClick={create_account}>create_account</button>
      <button className="h-10 w-min p-2 bg-blue-600 cursor-pointer" onClick={login}>login</button>

      {logged && <div>
        <p>Sync status: {syncStatus}</p>
      </div>}

    </div>
  )
}
