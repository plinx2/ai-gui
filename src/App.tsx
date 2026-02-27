import { useState } from "react";
import MainLayout from "./components/layout/MainLayout";
import SettingsView from "./components/settings/SettingsView";

type View = "chat" | "settings";

export default function App() {
  const [view, setView] = useState<View>("chat");

  return view === "settings" ? (
    <SettingsView onBack={() => setView("chat")} />
  ) : (
    <MainLayout onOpenSettings={() => setView("settings")} />
  );
}
