import { useState, useEffect } from "react";
import { usePlaybooks } from "../../hooks/usePlaybooks";
import { api } from "../../api";
import type { ModelInfo, Playbook, SessionSummary } from "../../types";

interface PlaybooksViewProps {
  onBack: () => void;
  onRun: (summary: SessionSummary) => void;
}

function newPlaybook(): Playbook {
  return {
    id: crypto.randomUUID(),
    title: "New Playbook",
    description: "",
    steps: [""],
    notes: "",
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
}

export default function PlaybooksView({ onBack, onRun }: PlaybooksViewProps) {
  const { playbooks, save, remove } = usePlaybooks();
  const [selected, setSelected] = useState<Playbook | null>(null);
  const [saving, setSaving] = useState(false);
  const [models, setModels] = useState<ModelInfo[]>([]);

  // Run dialog state
  const [showRunDialog, setShowRunDialog] = useState(false);
  const [userMessage, setUserMessage] = useState("");
  const [running, setRunning] = useState(false);
  const [runError, setRunError] = useState<string | null>(null);

  useEffect(() => {
    api.getModels().then(setModels).catch(console.error);
  }, []);

  const handleNew = () => setSelected(newPlaybook());
  const handleSelect = (pb: Playbook) => setSelected({ ...pb });

  const handleSave = async () => {
    if (!selected) return;
    setSaving(true);
    try {
      const updated = { ...selected, updatedAt: new Date().toISOString() };
      await save(updated);
      setSelected(updated);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (id: string) => {
    await remove(id);
    if (selected?.id === id) setSelected(null);
  };

  const handleOpenRunDialog = () => {
    setUserMessage("");
    setRunError(null);
    setShowRunDialog(true);
  };

  const handleRun = async () => {
    if (!selected || running) return;
    setRunning(true);
    setRunError(null);
    try {
      const response = await api.runPlaybook({
        playbookId: selected.id,
        userMessage: userMessage.trim() || undefined,
      });
      const freshSession = await api.getSession(response.sessionId);
      const summary: SessionSummary = {
        id: freshSession.id,
        title: freshSession.title,
        modelName: freshSession.modelName,
        updatedAt: freshSession.updatedAt,
        totalInputTokens: freshSession.totalInputTokens,
        totalOutputTokens: freshSession.totalOutputTokens,
      };
      setShowRunDialog(false);
      onRun(summary);
    } catch (e) {
      setRunError(String(e));
    } finally {
      setRunning(false);
    }
  };

  const updateStep = (idx: number, value: string) => {
    if (!selected) return;
    const steps = [...selected.steps];
    steps[idx] = value;
    setSelected({ ...selected, steps });
  };

  const addStep = () => {
    if (!selected) return;
    setSelected({ ...selected, steps: [...selected.steps, ""] });
  };

  const removeStep = (idx: number) => {
    if (!selected) return;
    const steps = selected.steps.filter((_, i) => i !== idx);
    setSelected({ ...selected, steps: steps.length > 0 ? steps : [""] });
  };

  const runnable =
    selected !== null &&
    selected.steps.filter((s) => s.trim().length > 0).length > 0 &&
    !!selected.modelId &&
    models.some((m) => m.isAvailable && m.id === selected.modelId);

  return (
    <div className="flex flex-col h-screen bg-slate-950 text-slate-100">
      {/* Header */}
      <div className="border-b border-slate-700 px-6 py-4 flex items-center gap-3 shrink-0">
        <button
          onClick={onBack}
          className="text-slate-400 hover:text-white transition-colors"
        >
          ← Back
        </button>
        <h1 className="text-lg font-semibold">Playbooks</h1>
      </div>

      {/* Two-panel layout */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left: list */}
        <aside className="w-64 border-r border-slate-700 flex flex-col shrink-0">
          <div className="p-3">
            <button
              onClick={handleNew}
              className="w-full flex items-center gap-2 px-3 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-medium transition-colors"
            >
              <span className="text-lg leading-none">+</span>
              New Playbook
            </button>
          </div>
          <ul className="flex-1 overflow-y-auto px-2 space-y-0.5">
            {playbooks.length === 0 && (
              <p className="text-slate-500 text-xs text-center py-6">
                No playbooks yet
              </p>
            )}
            {playbooks.map((pb) => (
              <li key={pb.id}>
                <button
                  onClick={() => handleSelect(pb)}
                  className={`w-full text-left px-3 py-2 rounded-lg text-sm transition-colors group relative ${
                    selected?.id === pb.id
                      ? "bg-slate-700 text-white"
                      : "text-slate-300 hover:bg-slate-800 hover:text-white"
                  }`}
                >
                  <span className="block truncate pr-6">{pb.title}</span>
                  <span className="block text-xs text-slate-500 mt-0.5">
                    {pb.steps.length} step{pb.steps.length !== 1 ? "s" : ""}
                    {pb.modelId && (
                      <span className="ml-1 text-indigo-400">· {pb.modelId}</span>
                    )}
                  </span>
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      handleDelete(pb.id);
                    }}
                    className="absolute right-2 top-1/2 -translate-y-1/2 opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-400 text-xs px-1 transition-opacity"
                    title="Delete"
                  >
                    ✕
                  </button>
                </button>
              </li>
            ))}
          </ul>
        </aside>

        {/* Right: editor */}
        {selected ? (
          <div className="flex-1 flex flex-col overflow-hidden">
            <div className="flex-1 overflow-y-auto p-6 space-y-6">
              {/* Title */}
              <div>
                <label className="block text-sm text-slate-400 mb-1.5">
                  Title
                </label>
                <input
                  type="text"
                  value={selected.title}
                  onChange={(e) =>
                    setSelected({ ...selected, title: e.target.value })
                  }
                  className="w-full bg-slate-800 border border-slate-600 rounded-lg px-4 py-2.5 text-sm text-slate-100 focus:outline-none focus:border-indigo-500 transition-colors"
                />
              </div>

              {/* Description */}
              <div>
                <label className="block text-sm text-slate-400 mb-1.5">
                  Description
                  <span className="ml-2 text-slate-600 font-normal text-xs">（任意）</span>
                </label>
                <textarea
                  value={selected.description}
                  onChange={(e) =>
                    setSelected({ ...selected, description: e.target.value })
                  }
                  rows={2}
                  placeholder="What does this playbook do?"
                  className="w-full bg-slate-800 border border-slate-600 rounded-lg px-4 py-2.5 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors resize-none"
                />
              </div>

              {/* Model */}
              <div>
                <label className="block text-sm text-slate-400 mb-1.5">
                  Model
                </label>
                <select
                  value={selected.modelId ?? ""}
                  onChange={(e) =>
                    setSelected({
                      ...selected,
                      modelId: e.target.value || undefined,
                    })
                  }
                  className="w-full bg-slate-800 border border-slate-600 rounded-lg px-4 py-2.5 text-sm text-slate-100 focus:outline-none focus:border-indigo-500 transition-colors"
                >
                  <option value="">— Select a model —</option>
                  {models.map((m) => (
                    <option key={m.id} value={m.id} disabled={!m.isAvailable}>
                      {m.displayName}
                      {!m.isAvailable ? " (API key missing)" : ""}
                    </option>
                  ))}
                </select>
              </div>

              {/* Steps */}
              <div>
                <label className="block text-sm text-slate-400 mb-2">
                  Steps
                </label>
                <div className="space-y-3">
                  {selected.steps.map((step, idx) => (
                    <div key={idx} className="flex gap-2 items-start">
                      <span className="text-xs text-slate-500 pt-3 w-5 shrink-0 text-right">
                        {idx + 1}.
                      </span>
                      <textarea
                        value={step}
                        onChange={(e) => updateStep(idx, e.target.value)}
                        rows={2}
                        placeholder="Enter step description…"
                        className="flex-1 bg-slate-800 border border-slate-600 rounded-lg px-3 py-2 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors resize-none"
                      />
                      <button
                        onClick={() => removeStep(idx)}
                        className="text-slate-500 hover:text-red-400 text-xs pt-2.5 transition-colors shrink-0"
                        title="Remove step"
                      >
                        ✕
                      </button>
                    </div>
                  ))}
                </div>
                <button
                  onClick={addStep}
                  className="mt-3 text-sm text-indigo-400 hover:text-indigo-300 transition-colors"
                >
                  + Add Step
                </button>
              </div>

              {/* Notes */}
              <div>
                <label className="block text-sm text-slate-400 mb-1.5">
                  Notes
                  <span className="ml-2 text-slate-600 font-normal text-xs">（注意事項）</span>
                </label>
                <textarea
                  value={selected.notes}
                  onChange={(e) =>
                    setSelected({ ...selected, notes: e.target.value })
                  }
                  rows={4}
                  placeholder="Add any notes, constraints, or caveats…"
                  className="w-full bg-slate-800 border border-slate-600 rounded-lg px-4 py-2.5 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors resize-none"
                />
              </div>
            </div>

            {/* Footer */}
            <div className="border-t border-slate-700 px-6 py-4 shrink-0 flex items-center justify-end gap-3">
              <button
                onClick={handleSave}
                disabled={saving}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 disabled:bg-slate-800 rounded-lg text-sm font-medium text-slate-200 transition-colors"
              >
                {saving ? "Saving…" : "Save"}
              </button>
              <button
                onClick={handleOpenRunDialog}
                disabled={!runnable}
                title={
                  !selected.modelId
                    ? "Select a model to run this playbook"
                    : !runnable
                    ? "Add at least one step to run"
                    : undefined
                }
                className="px-5 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:bg-slate-700 disabled:text-slate-500 rounded-lg text-sm font-medium text-white transition-colors"
              >
                ▶ Run
              </button>
            </div>
          </div>
        ) : (
          <div className="flex-1 flex items-center justify-center text-slate-500 text-sm">
            Select a playbook or create a new one
          </div>
        )}
      </div>

      {/* Run dialog */}
      {showRunDialog && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
          <div className="bg-slate-900 border border-slate-700 rounded-xl w-full max-w-lg mx-4 shadow-2xl">
            <div className="px-6 py-4 border-b border-slate-700">
              <h2 className="text-base font-semibold text-slate-100">
                Run: {selected?.title}
              </h2>
              <p className="text-xs text-slate-400 mt-0.5">
                追加のメッセージ（任意）。省略するとPlaybookの内容だけで実行されます。
              </p>
            </div>
            <div className="p-6">
              <label className="block text-sm text-slate-400 mb-1.5">
                Message
                <span className="ml-2 text-slate-600 font-normal text-xs">（optional）</span>
              </label>
              <textarea
                value={userMessage}
                onChange={(e) => setUserMessage(e.target.value)}
                rows={5}
                autoFocus
                placeholder="e.g. Write a blog post about the benefits of Rust…"
                className="w-full bg-slate-800 border border-slate-600 rounded-lg px-4 py-3 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors resize-none"
                onKeyDown={(e) => {
                  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                    e.preventDefault();
                    if (!running) handleRun();
                  }
                }}
              />
              {runError && (
                <p className="mt-2 text-sm text-red-400">{runError}</p>
              )}
            </div>
            <div className="px-6 py-4 border-t border-slate-700 flex items-center justify-end gap-3">
              <button
                onClick={() => setShowRunDialog(false)}
                disabled={running}
                className="px-4 py-2 bg-slate-700 hover:bg-slate-600 disabled:opacity-50 rounded-lg text-sm font-medium text-slate-200 transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={handleRun}
                disabled={running}
                className="px-5 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:bg-slate-700 disabled:text-slate-500 rounded-lg text-sm font-medium text-white transition-colors"
              >
                {running ? "Running…" : "▶ Run"}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
