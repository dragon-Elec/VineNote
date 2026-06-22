import { useEffect } from "react";
import SideBar from "./components/side-bar";
import Editor from "./components/editor";
import { SettingsProvider } from "./components/settings";
import { useSelectedFile } from "./components/notes-list/controllers/selected-file";
import { useAppMode } from "./components/editor/controllers/app-mode";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

function App() {
  const setSelectedFile = useSelectedFile((state) => state.setSelectedFile);
  const setAppMode = useAppMode((state) => state.setAppMode);

  useEffect(() => {
    invoke<string | null>("get_cli_arg_file")
      .then((filePath) => {
        if (filePath) {
          setAppMode("StandaloneEdit");
          setSelectedFile({
            id: "standalone",
            name: filePath.replace(/\\/g, "/").split("/").pop() || "Untitled",
            path: filePath,
            metadata: {
              is_file: true,
              is_dir: false,
              len: 0,
              created: "",
            },
          });
        }
      })
      .catch((err) => {
        console.error("Failed to get cli arg file", err);
      });
  }, [setSelectedFile, setAppMode]);

  return (
    <SettingsProvider>
      <main className="main">
        <SideBar />
        <Editor />
      </main>
    </SettingsProvider>
  );
}

export default App;
