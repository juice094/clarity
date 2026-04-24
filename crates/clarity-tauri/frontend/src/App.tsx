import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");
  const [version, setVersion] = useState("");

  async function greet() {
    const msg = await invoke<string>("greet", { name });
    setGreetMsg(msg);
  }

  async function fetchVersion() {
    const ver = await invoke<string>("get_app_version");
    setVersion(ver);
  }

  return (
    <main className="container">
      <h1>Clarity</h1>
      <p className="tagline">Personal AI Standard Runtime</p>

      <div className="card">
        <input
          id="greet-input"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <button type="button" onClick={() => greet()}>
          Greet
        </button>
        <p>{greetMsg}</p>
      </div>

      <div className="card">
        <button type="button" onClick={() => fetchVersion()}>
          Get Version
        </button>
        <p>{version && `Version: ${version}`}</p>
      </div>

      <p className="status">
        Desktop GUI powered by Tauri 2 + React
      </p>
    </main>
  );
}

export default App;
