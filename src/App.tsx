import { useState } from "react";
import MainLayout from "./components/layout/MainLayout";
import SettingsView from "./components/settings/SettingsView";
import PlaybooksView from "./components/playbook/PlaybooksView";
import type { SessionSummary } from "./types";

type View = "chat" | "settings" | "playbooks";

interface PlaybookRun {
  summary: SessionSummary;
  key: number;
}

export default function App() {
  const [view, setView] = useState<View>("chat");
  const [playbookRun, setPlaybookRun] = useState<PlaybookRun | null>(null);

  const handleRunPlaybook = (summary: SessionSummary) => {
    setPlaybookRun({ summary, key: Date.now() });
    setView("chat");
  };

  if (view === "settings") {
    return <SettingsView onBack={() => setView("chat")} />;
  }

  if (view === "playbooks") {
    return (
      <PlaybooksView
        onBack={() => setView("chat")}
        onRun={handleRunPlaybook}
      />
    );
  }

  return (
    <MainLayout
      onOpenSettings={() => setView("settings")}
      onOpenPlaybooks={() => setView("playbooks")}
      playbookRun={playbookRun}
    />
  );
}
