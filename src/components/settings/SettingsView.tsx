import { useState, useEffect } from "react";
import { api } from "../../api";
import type { Config, ConfigField } from "../../types";

interface SettingsViewProps {
  onBack: () => void;
}

export default function SettingsView({ onBack }: SettingsViewProps) {
  const [config, setConfig] = useState<Config>({ settings: {} });
  const [schema, setSchema] = useState<ConfigField[]>([]);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [configPath, setConfigPath] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([
      api.getConfig(),
      api.getConfigSchema(),
      api.getConfigPath(),
    ])
      .then(([cfg, schemaFields, path]) => {
        setConfig(cfg);
        setSchema(schemaFields);
        setConfigPath(path);
      })
      .catch(console.error);
  }, []);

  const handleFieldChange = (key: string, value: string) => {
    setConfig((prev) => ({
      settings: { ...prev.settings, [key]: value },
    }));
  };

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    setSaved(false);
    try {
      await api.updateConfig(config);
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

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
        <h1 className="text-lg font-semibold">Settings</h1>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        <div className="max-w-xl mx-auto px-6 py-8 space-y-8">
          {/* API Keys section — dynamic fields from model registry */}
          <section>
            <h2 className="text-sm font-semibold text-slate-400 uppercase tracking-wider mb-4">
              API Keys
            </h2>
            {schema.length === 0 ? (
              <p className="text-sm text-slate-500">No models registered.</p>
            ) : (
              <div className="space-y-4">
                {schema.map((field) => (
                  <div key={field.key}>
                    <label className="block text-sm text-slate-300 mb-1.5">
                      {field.label}
                    </label>
                    <input
                      type={field.isSecret ? "password" : "text"}
                      value={config.settings[field.key] ?? ""}
                      onChange={(e) => handleFieldChange(field.key, e.target.value)}
                      placeholder={field.placeholder}
                      className="w-full bg-slate-800 border border-slate-600 rounded-lg px-4 py-2.5 text-sm text-slate-100 placeholder-slate-500 focus:outline-none focus:border-indigo-500 transition-colors"
                    />
                  </div>
                ))}
              </div>
            )}
          </section>

          {/* Storage paths */}
          {configPath && (
            <section>
              <h2 className="text-sm font-semibold text-slate-400 uppercase tracking-wider mb-4">
                Storage
              </h2>
              <div className="space-y-2">
                <p className="text-xs text-slate-500">Config file</p>
                <p className="text-xs text-slate-300 font-mono bg-slate-800 px-3 py-2 rounded-lg break-all select-all">
                  {configPath}
                </p>
              </div>
            </section>
          )}

          {/* Error */}
          {error && (
            <div className="px-4 py-3 bg-red-900/50 border border-red-700 rounded-lg text-red-300 text-sm">
              {error}
            </div>
          )}
        </div>
      </div>

      {/* Footer */}
      <div className="border-t border-slate-700 px-6 py-4 shrink-0 flex items-center justify-end gap-3">
        {saved && <span className="text-green-400 text-sm">✓ Saved</span>}
        <button
          onClick={handleSave}
          disabled={saving}
          className="px-5 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:bg-slate-700 rounded-lg text-sm font-medium text-white transition-colors"
        >
          {saving ? "Saving…" : "Save"}
        </button>
      </div>
    </div>
  );
}
