import SideBar from "./components/side-bar";
import Editor from "./components/editor";
import PipelineDetail from "./components/pipeline";
import { SettingsProvider } from "./components/settings";
import { useSelectedNav } from "./components/navigation-bar/controllers/selected-nav";
import { RightPanel } from "./components/right-panel";
import "./App.css";

const PIPELINE_MODES = ["sources", "inbox", "cards"];

function AppContent() {
  const selectedNav = useSelectedNav((s) => s.selectedNav);
  const isPipelineMode = PIPELINE_MODES.includes(selectedNav);

  return (
    <main className="main">
      <SideBar />
      {isPipelineMode ? <PipelineDetail /> : <Editor />}
      <RightPanel />
    </main>
  );
}

function App() {
  return (
    <SettingsProvider>
      <AppContent />
    </SettingsProvider>
  );
}

export default App;
