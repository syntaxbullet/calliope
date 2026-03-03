import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "../../store";
import { X, XCircle, Download, Trash2, Check, Save } from "lucide-react";
import type { AudioDevice, LinuxInjectionStatus, ModelInfo, Settings } from "../../types";

type Tab = "general" | "models" | "customization" | "postprocessing" | "hotkeys";

// ── Custom form controls ─────────────────────────────────────────────────────

function Checkbox({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
}) {
  return (
    <label
      style={{
        display: "flex",
        alignItems: "center",
        gap: 8,
        fontSize: 13,
        cursor: "pointer",
        userSelect: "none",
      }}
      onClick={(e) => {
        e.preventDefault();
        onChange(!checked);
      }}
    >
      <span
        style={{
          width: 16,
          height: 16,
          borderRadius: 4,
          border: checked ? "1.5px solid var(--text-primary)" : "1.5px solid var(--border-strong)",
          background: checked ? "var(--text-primary)" : "var(--bg-surface)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
          transition: "all 0.15s ease",
        }}
      >
        {checked && (
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
            <path
              d="M2 5.5L4 7.5L8 3"
              stroke={checked ? "var(--bg-base)" : "none"}
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        )}
      </span>
      {label}
    </label>
  );
}

function Radio({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: () => void;
  label: string;
  name?: string;
}) {
  return (
    <label
      style={{
        display: "flex",
        alignItems: "center",
        gap: 6,
        fontSize: 13,
        cursor: "pointer",
        userSelect: "none",
      }}
      onClick={(e) => {
        e.preventDefault();
        onChange();
      }}
    >
      <span
        style={{
          width: 16,
          height: 16,
          borderRadius: "50%",
          border: checked ? "1.5px solid var(--text-primary)" : "1.5px solid var(--border-strong)",
          background: "var(--bg-surface)",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          flexShrink: 0,
          transition: "all 0.15s ease",
        }}
      >
        {checked && (
          <span
            style={{
              width: 8,
              height: 8,
              borderRadius: "50%",
              background: "var(--text-primary)",
            }}
          />
        )}
      </span>
      {label}
    </label>
  );
}

interface DownloadProgressEvent {
  name: string;
  bytes_downloaded: number;
  total_bytes: number;
}

function formatBytes(n: number): string {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)} GB`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(0)} MB`;
  return `${n} B`;
}

export function SettingsPanel() {
  const { settings } = useAppStore();
  const [tab, setTab] = useState<Tab>("general");

  // Local copies so we can edit without immediately saving
  const [local, setLocal] = useState<Settings>(settings);
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [downloading, setDownloading] = useState<Record<string, number>>({}); // name → 0–100
  const [saveMsg, setSaveMsg] = useState("");

  // Sync local state when store settings change (from settings-changed event)
  useEffect(() => {
    setLocal(settings);
  }, [settings]);

  useEffect(() => {
    invoke<AudioDevice[]>("list_audio_devices").then(setDevices).catch(console.error);
    invoke<ModelInfo[]>("list_models").then(setModels).catch(console.error);
  }, []);

  // Listen for download progress events
  useEffect(() => {
    const unlisten = listen<DownloadProgressEvent>("download-progress", (e) => {
      const { name, bytes_downloaded, total_bytes } = e.payload;
      const pct = total_bytes > 0 ? Math.round((bytes_downloaded / total_bytes) * 100) : 0;
      setDownloading((prev) => ({ ...prev, [name]: pct }));
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  function patch(partial: Partial<Settings>) {
    setLocal((prev) => ({ ...prev, ...partial }));
  }

  async function save() {
    try {
      // Merge local edits onto the latest store settings to preserve
      // fields changed by other commands (e.g. active_model from set_active_model)
      const merged = { ...settings, ...local };
      await invoke("save_settings", { settings: merged });
      // settings-changed event will update the store and local via the useEffect
      setSaveMsg("Saved");
      setTimeout(() => setSaveMsg(""), 1500);
    } catch (e) {
      setSaveMsg(`Error: ${e}`);
    }
  }

  async function saveHotkeys() {
    try {
      await invoke("update_hotkeys", {
        hotkeyPtt: local.hotkey_ptt,
        hotkeyToggle: local.hotkey_toggle,
      });
      // settings-changed event will update the store and local via the useEffect
      setSaveMsg("Saved");
      setTimeout(() => setSaveMsg(""), 1500);
    } catch (e) {
      setSaveMsg(`Error: ${e}`);
    }
  }

  async function selectModel(name: string | null) {
    try {
      await invoke("set_active_model", { name });
      // set_active_model calls save() which emits settings-changed,
      // so the store and local state will sync automatically.
      // But also update local immediately to avoid stale overwrites.
      setLocal((prev) => ({ ...prev, active_model: name }));
      const updated = await invoke<ModelInfo[]>("list_models");
      setModels(updated);
    } catch (e) {
      console.error(e);
    }
  }

  async function deleteModel(name: string) {
    try {
      await invoke("delete_model", { name });
      const updated = await invoke<ModelInfo[]>("list_models");
      setModels(updated);
    } catch (e) {
      console.error(e);
    }
  }

  async function cancelDownload() {
    try {
      await invoke("cancel_download");
    } catch (e) {
      console.error(e);
    }
  }

  async function downloadModel(name: string) {
    setDownloading((prev) => ({ ...prev, [name]: 0 }));
    try {
      await invoke("download_model", { name });
      const updated = await invoke<ModelInfo[]>("list_models");
      setModels(updated);
    } catch (e) {
      console.error(e);
    } finally {
      setDownloading((prev) => {
        const next = { ...prev };
        delete next[name];
        return next;
      });
    }
  }

  const tabStyle = (t: Tab): React.CSSProperties => ({
    padding: "6px 14px",
    fontSize: 13,
    fontFamily: "var(--font-sans)",
    border: "none",
    background: "none",
    cursor: "pointer",
    color: tab === t ? "var(--text-primary)" : "var(--text-secondary)",
    borderBottom: tab === t ? "2px solid var(--text-primary)" : "2px solid transparent",
    transition: "color 0.15s",
  });

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "var(--bg-base)",
        color: "var(--text-primary)",
        display: "flex",
        flexDirection: "column",
        fontFamily: "var(--font-sans)",
      }}
    >
      {/* Header — drag region for frameless window */}
      <div
        data-tauri-drag-region
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "12px 20px 0",
          borderBottom: "1px solid var(--border)",
        }}
      >
        <div style={{ display: "flex", gap: 4 }}>
          {(["general", "models", "postprocessing", "customization", "hotkeys"] as Tab[]).map((t) => (
            <button key={t} style={tabStyle(t)} onClick={() => setTab(t)}>
              {t === "postprocessing" ? "Post-processing" : t.charAt(0).toUpperCase() + t.slice(1)}
            </button>
          ))}
        </div>
        <button
          onClick={() => getCurrentWindow().hide()}
          style={{
            background: "none",
            border: "none",
            cursor: "pointer",
            color: "var(--text-secondary)",
            fontSize: 18,
            lineHeight: 1,
            padding: "0 4px",
          }}
          aria-label="Close settings"
        >
          <X size={16} strokeWidth={1.5} />
        </button>
      </div>

      {/* Body */}
      <div style={{ flex: 1, overflowY: "auto", padding: "20px 24px" }}>
        {tab === "general" && (
          <GeneralTab local={local} patch={patch} devices={devices} onSave={save} saveMsg={saveMsg} />
        )}
        {tab === "models" && (
          <ModelsTab models={models} downloading={downloading} onDownload={downloadModel} onSelect={selectModel} onDelete={deleteModel} onCancelDownload={cancelDownload} />
        )}
        {tab === "customization" && (
          <CustomizationTab local={local} patch={patch} onSave={save} saveMsg={saveMsg} />
        )}
        {tab === "postprocessing" && (
          <PostProcessingTab local={local} patch={patch} onSave={save} saveMsg={saveMsg} />
        )}
{tab === "hotkeys" && (
          <HotkeysTab local={local} patch={patch} onSave={saveHotkeys} saveMsg={saveMsg} />
        )}
      </div>
    </div>
  );
}

// ── General tab ───────────────────────────────────────────────────────────────

function AccessibilityBanner() {
  const [trusted, setTrusted] = useState<boolean | null>(null);

  useEffect(() => {
    invoke<boolean>("check_accessibility").then(setTrusted).catch(() => setTrusted(true));
  }, []);

  if (trusted === null || trusted) return null;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        justifyContent: "space-between",
        padding: "10px 14px",
        borderRadius: 8,
        background: "var(--bg-surface)",
        border: "1px solid var(--border)",
        fontSize: 13,
      }}
    >
      <span style={{ color: "var(--text-secondary)" }}>
        Accessibility permission is required for text injection
      </span>
      <button
        onClick={() => {
          invoke<boolean>("request_accessibility").then((ok) => {
            if (ok) setTrusted(true);
          });
        }}
        style={btnStyle}
      >
        Grant Access
      </button>
    </div>
  );
}

function LinuxInjectionBanner() {
  const [status, setStatus] = useState<LinuxInjectionStatus | null>(null);

  useEffect(() => {
    invoke<LinuxInjectionStatus | null>("check_linux_injection_status")
      .then(setStatus)
      .catch(() => setStatus(null));
  }, []);

  if (!status || !status.recommended_action) return null;

  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        padding: "10px 14px",
        borderRadius: 8,
        background: "var(--bg-surface)",
        border: "1px solid var(--border)",
        fontSize: 13,
        color: "var(--text-secondary)",
      }}
    >
      {status.recommended_action}
    </div>
  );
}

function GeneralTab({
  local,
  patch,
  devices,
  onSave,
  saveMsg,
}: {
  local: Settings;
  patch: (p: Partial<Settings>) => void;
  devices: AudioDevice[];
  onSave: () => void;
  saveMsg: string;
}) {
  const [backend, setBackend] = useState<string | null>(null);
  const [gpuInstalled, setGpuInstalled] = useState<boolean | null>(null);
  const [gpuDownloading, setGpuDownloading] = useState<number | null>(null);
  const [gpuDownloadError, setGpuDownloadError] = useState<string | null>(null);

  useEffect(() => {
    invoke<string>("get_acceleration_backend").then(setBackend).catch(() => setBackend("CPU"));
    invoke<boolean>("check_whisper_cli_available").then(setGpuInstalled).catch(() => setGpuInstalled(false));
  }, []);

  // Listen for GPU binary download progress
  useEffect(() => {
    if (!backend) return;
    const progressName = `whisper-cli-${backend}`;
    const unlisten = listen<DownloadProgressEvent>("download-progress", (e) => {
      const { name, bytes_downloaded, total_bytes } = e.payload;
      if (name === progressName) {
        const pct = total_bytes > 0 ? Math.round((bytes_downloaded / total_bytes) * 100) : 0;
        setGpuDownloading(pct);
      }
    });
    return () => { unlisten.then((f) => f()); };
  }, [backend]);

  const needsDownload = backend !== null && gpuInstalled === false;
  const hasGpuBackend = backend !== null && backend !== "CPU";

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <AccessibilityBanner />
      <LinuxInjectionBanner />
      <Field label="Input device">
        <select
          value={local.audio_device ?? ""}
          onChange={(e) => patch({ audio_device: e.target.value || null })}
          style={selectStyle}
        >
          <option value="">System default</option>
          {devices.map((d) => (
            <option key={d.id} value={d.id}>
              {d.name}
            </option>
          ))}
        </select>
      </Field>

      <Field label="Text insertion mode">
        <div style={{ display: "flex", gap: 16 }}>
          <Radio
            name="insertion_mode"
            checked={local.injection_mode === "Clipboard"}
            onChange={() => patch({ injection_mode: "Clipboard" })}
            label="Clipboard"
          />
          <Radio
            name="insertion_mode"
            checked={local.injection_mode === "Character"}
            onChange={() => patch({ injection_mode: "Character" })}
            label="Character by character"
          />
        </div>
        <span style={{ fontSize: 11, color: "var(--text-secondary)", marginTop: 2 }}>
          {local.injection_mode === "Clipboard"
            ? "Pastes transcribed text via the clipboard (faster, overwrites clipboard)"
            : "Types text character by character (slower, preserves clipboard)"}
        </span>
      </Field>

      <Field label="Language">
        <select
          value={local.language}
          onChange={(e) => patch({ language: e.target.value })}
          style={selectStyle}
        >
          {WHISPER_LANGUAGES.map((l) => (
            <option key={l.code} value={l.code}>
              {l.name}{l.code !== "auto" ? ` (${l.code})` : ""}
            </option>
          ))}
        </select>
      </Field>

      <Field label="Silence timeout (toggle mode)">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <input
            type="number"
            min={0}
            max={30}
            step={0.5}
            value={local.silence_timeout_secs}
            onChange={(e) => patch({ silence_timeout_secs: parseFloat(e.target.value) || 0 })}
            style={{ ...inputStyle, width: 80 }}
          />
          <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>seconds</span>
        </div>
        <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
          In toggle mode, recording stops automatically after this duration of silence. Set to 0 to disable.
        </span>
      </Field>

      <Field label="Silence threshold">
        <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
          <input
            type="range"
            min={0.01}
            max={0.15}
            step={0.01}
            value={local.silence_threshold}
            onChange={(e) => patch({ silence_threshold: parseFloat(e.target.value) })}
            style={{ width: 160 }}
          />
          <span style={{ fontSize: 12, color: "var(--text-secondary)", fontFamily: "var(--font-mono)", minWidth: 36 }}>
            {local.silence_threshold.toFixed(2)}
          </span>
        </div>
        <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
          How quiet audio must be to count as silence. Raise this if silence timeout never triggers due to background noise.
        </span>
      </Field>

      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          {hasGpuBackend && (
            <Checkbox
              checked={local.use_gpu}
              onChange={(v) => patch({ use_gpu: v })}
              label="Hardware acceleration"
            />
          )}
          {backend && (
            <span
              style={{
                fontSize: 11,
                fontFamily: "var(--font-mono)",
                padding: "2px 8px",
                borderRadius: 4,
                background: "var(--bg-surface)",
                border: "1px solid var(--border)",
                color: "var(--text-secondary)",
              }}
            >
              {backend}
            </span>
          )}
          {gpuInstalled && (
            <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
              <Check size={12} style={{ display: "inline", verticalAlign: "middle", marginRight: 2 }} />
              whisper-cli installed
            </span>
          )}
        </div>
        {needsDownload && gpuDownloading === null && (
          <button
            onClick={async () => {
              setGpuDownloadError(null);
              setGpuDownloading(0);
              try {
                await invoke("download_gpu_backend", { backend });
                setGpuInstalled(true);
                setGpuDownloading(null);
              } catch (e: any) {
                setGpuDownloadError(String(e));
                setGpuDownloading(null);
              }
            }}
            style={{
              fontSize: 12,
              padding: "6px 12px",
              borderRadius: 6,
              border: "1px solid var(--border)",
              background: "var(--bg-surface)",
              color: "var(--text-primary)",
              cursor: "pointer",
              display: "flex",
              alignItems: "center",
              gap: 6,
              width: "fit-content",
            }}
          >
            <Download size={14} />
            Download whisper-cli ({backend})
          </button>
        )}
        {gpuDownloading !== null && (
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <div style={{ width: 160, height: 6, borderRadius: 3, background: "var(--bg-surface)", overflow: "hidden" }}>
              <div style={{ width: `${gpuDownloading}%`, height: "100%", borderRadius: 3, background: "var(--text-secondary)", transition: "width 0.2s" }} />
            </div>
            <span style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "var(--font-mono)" }}>{gpuDownloading}%</span>
          </div>
        )}
        {gpuDownloadError && (
          <span style={{ fontSize: 11, color: "#e55" }}>{gpuDownloadError}</span>
        )}
      </div>

      <Checkbox
        checked={local.launch_at_login}
        onChange={async (v) => {
          try {
            await invoke("set_launch_at_login", { enabled: v });
            patch({ launch_at_login: v });
          } catch (e) {
            console.error(e);
          }
        }}
        label="Launch at login"
      />

      <Checkbox
        checked={local.debug_logs}
        onChange={(v) => patch({ debug_logs: v })}
        label="Enable debug logs"
      />

      <SaveRow onSave={onSave} msg={saveMsg} />
    </div>
  );
}

// ── Models tab ────────────────────────────────────────────────────────────────

function ModelsTab({
  models,
  downloading,
  onDownload,
  onSelect,
  onDelete,
  onCancelDownload,
}: {
  models: ModelInfo[];
  downloading: Record<string, number>;
  onDownload: (name: string) => void;
  onSelect: (name: string | null) => void;
  onDelete: (name: string) => void;
  onCancelDownload: () => void;
}) {
  const downloaded = models.filter((m) => m.downloaded);
  const notDownloaded = models.filter((m) => !m.downloaded);

  function ModelCard({ m }: { m: ModelInfo }) {
    const pct = downloading[m.name];
    const isDownloading = pct !== undefined;
    const grayed = !m.downloaded && !isDownloading;

    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "10px 14px",
          borderRadius: 8,
          background: "var(--bg-surface)",
          border: m.active ? "1px solid var(--text-primary)" : "1px solid var(--border)",
          opacity: 1,
        }}
      >
        <div style={{ opacity: grayed ? 0.45 : 1 }}>
          <p style={{ margin: 0, fontSize: 13, fontWeight: 500 }}>{m.name}</p>
          <p style={{ margin: 0, fontSize: 12, color: "var(--text-secondary)" }}>
            {m.speed_label} · {m.quality_label} · {formatBytes(m.size_bytes)}
          </p>
          {isDownloading && (
            <div style={{ marginTop: 6, width: 160, height: 4, background: "var(--border)", borderRadius: 2 }}>
              <div style={{ width: `${pct}%`, height: "100%", background: "var(--text-primary)", borderRadius: 2, transition: "width 0.2s" }} />
            </div>
          )}
        </div>
        {!m.downloaded && !isDownloading && (
          <button onClick={() => onDownload(m.name)} style={{ ...btnStyle, display: "flex", alignItems: "center", gap: 4 }}>
            <Download size={14} strokeWidth={1.5} /> Download
          </button>
        )}
        {m.downloaded && !m.active && (
          <div style={{ display: "flex", gap: 6 }}>
            <button onClick={() => onSelect(m.name)} style={{ ...btnStyle, display: "flex", alignItems: "center", gap: 4 }}>
              <Check size={14} strokeWidth={1.5} /> Use
            </button>
            <button onClick={() => onDelete(m.name)} style={{ ...btnStyle, display: "flex", alignItems: "center", gap: 4 }}>
              <Trash2 size={14} strokeWidth={1.5} /> Delete
            </button>
          </div>
        )}
        {m.downloaded && m.active && (
          <div style={{ display: "flex", gap: 6 }}>
            <button onClick={() => onSelect(null)} style={btnStyle}>
              Unload
            </button>
            <button onClick={() => { onSelect(null); onDelete(m.name); }} style={{ ...btnStyle, display: "flex", alignItems: "center", gap: 4 }}>
              <Trash2 size={14} strokeWidth={1.5} /> Delete
            </button>
          </div>
        )}
        {isDownloading && (
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{pct}%</span>
            <button
              onClick={onCancelDownload}
              style={{ ...btnStyle, display: "flex", alignItems: "center", gap: 4, padding: "3px 8px" }}
              title="Cancel download"
            >
              <XCircle size={14} strokeWidth={1.5} />
            </button>
          </div>
        )}
      </div>
    );
  }

  const sectionLabel: React.CSSProperties = {
    margin: 0, fontSize: 11, color: "var(--text-secondary)", textTransform: "uppercase", letterSpacing: 0.5,
  };

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
      {downloaded.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <p style={sectionLabel}>Downloaded</p>
          {downloaded.map((m) => <ModelCard key={m.name} m={m} />)}
        </div>
      )}

      {notDownloaded.length > 0 && (
        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          <p style={sectionLabel}>Available</p>
          {notDownloaded.map((m) => <ModelCard key={m.name} m={m} />)}
        </div>
      )}
    </div>
  );
}

// ── Customization tab ─────────────────────────────────────────────────────────

function CustomizationTab({
  local,
  patch,
  onSave,
  saveMsg,
}: {
  local: Settings;
  patch: (p: Partial<Settings>) => void;
  onSave: () => void;
  saveMsg: string;
}) {
  const [newWord, setNewWord] = useState("");

  function addWord() {
    const word = newWord.trim();
    if (!word || local.custom_dictionary.includes(word)) return;
    patch({ custom_dictionary: [...local.custom_dictionary, word] });
    setNewWord("");
  }

  function removeWord(word: string) {
    patch({ custom_dictionary: local.custom_dictionary.filter((w) => w !== word) });
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <Field label="Whisper prompt">
        <textarea
          value={local.whisper_prompt}
          onChange={(e) => patch({ whisper_prompt: e.target.value })}
          placeholder="Optional prompt to guide transcription style, vocabulary, etc."
          rows={3}
          style={{ ...selectStyle, resize: "vertical", fontFamily: "var(--font-sans)" }}
        />
        <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
          Provide context to improve transcription accuracy (e.g. topic, expected vocabulary).
        </span>
      </Field>

      <Field label="Custom dictionary">
        <span style={{ fontSize: 11, color: "var(--text-secondary)", marginBottom: 4 }}>
          Add words, names, or terms that Whisper should recognize. These are appended to the whisper prompt automatically.
        </span>
        <div style={{ display: "flex", gap: 6 }}>
          <input
            type="text"
            value={newWord}
            onChange={(e) => setNewWord(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                addWord();
              }
            }}
            placeholder="Type a word and press Enter"
            style={{ ...inputStyle, flex: 1 }}
          />
          <button
            onClick={addWord}
            style={{
              ...btnStyle,
              opacity: newWord.trim() ? 1 : 0.5,
            }}
          >
            Add
          </button>
        </div>
        {local.custom_dictionary.length > 0 && (
          <div
            style={{
              display: "flex",
              flexWrap: "wrap",
              gap: 6,
              marginTop: 4,
            }}
          >
            {local.custom_dictionary.map((word) => (
              <span
                key={word}
                style={{
                  display: "inline-flex",
                  alignItems: "center",
                  gap: 4,
                  padding: "3px 8px",
                  borderRadius: 6,
                  border: "1px solid var(--border)",
                  background: "var(--bg-surface)",
                  fontSize: 12,
                  fontFamily: "var(--font-mono)",
                }}
              >
                {word}
                <span
                  onClick={() => removeWord(word)}
                  style={{
                    cursor: "pointer",
                    color: "var(--text-disabled)",
                    lineHeight: 1,
                    marginLeft: 2,
                    display: "inline-flex",
                  }}
                  role="button"
                  aria-label={`Remove ${word}`}
                >
                  <X size={12} strokeWidth={1.5} />
                </span>
              </span>
            ))}
          </div>
        )}
      </Field>

      <SaveRow onSave={onSave} msg={saveMsg} />
    </div>
  );
}

// ── Post-processing tab ──────────────────────────────────────────────────────

function PostProcessingTab({
  local,
  patch,
  onSave,
  saveMsg,
}: {
  local: Settings;
  patch: (p: Partial<Settings>) => void;
  onSave: () => void;
  saveMsg: string;
}) {
  const [apiKey, setApiKey] = useState("");
  const [apiKeyLoaded, setApiKeyLoaded] = useState(false);
  const [apiKeySaveMsg, setApiKeySaveMsg] = useState("");

  // Load existing API key status on mount
  useEffect(() => {
    invoke<string | null>("get_api_key", { provider: "openrouter" })
      .then((key) => {
        if (key) {
          setApiKey("••••••••••••••••");
          setApiKeyLoaded(true);
        }
      })
      .catch(console.error);
  }, []);

  async function saveApiKey() {
    if (!apiKey || apiKey === "••••••••••••••••") return;
    try {
      await invoke("save_api_key", { provider: "openrouter", key: apiKey });
      setApiKey("••••••••••••••••");
      setApiKeyLoaded(true);
      setApiKeySaveMsg("Saved");
      setTimeout(() => setApiKeySaveMsg(""), 1500);
    } catch (e) {
      setApiKeySaveMsg(`Error: ${e}`);
    }
  }

  async function deleteApiKey() {
    try {
      await invoke("delete_api_key", { provider: "openrouter" });
      setApiKey("");
      setApiKeyLoaded(false);
      setApiKeySaveMsg("Removed");
      setTimeout(() => setApiKeySaveMsg(""), 1500);
    } catch (e) {
      setApiKeySaveMsg(`Error: ${e}`);
    }
  }

  const provider = local.postprocess_provider;

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <p style={{ margin: 0, fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5 }}>
        Post-processing runs your transcription through an LLM before inserting it. Use it to fix
        grammar and punctuation, reformat dictated text into prose, expand shorthand, or apply
        custom writing rules. You can use a local model (Ollama or LM Studio) or a cloud provider
        (OpenRouter).
      </p>

      <Checkbox
        checked={local.postprocess_enabled}
        onChange={(v) => patch({ postprocess_enabled: v })}
        label="Enable post-processing"
      />

      {local.postprocess_enabled && (
        <>
          <Field label="Provider">
            <div style={{ display: "flex", gap: 16 }}>
              <Radio
                checked={provider === "Ollama"}
                onChange={() => patch({ postprocess_provider: "Ollama" })}
                label="Ollama (local)"
              />
              <Radio
                checked={provider === "LmStudio"}
                onChange={() => patch({ postprocess_provider: "LmStudio" })}
                label="LM Studio (local)"
              />
              <Radio
                checked={provider === "OpenRouter"}
                onChange={() => patch({ postprocess_provider: "OpenRouter" })}
                label="OpenRouter (cloud)"
              />
            </div>
          </Field>

          {provider === "Ollama" && (
            <>
              <Field label="Ollama endpoint">
                <input
                  type="text"
                  value={local.ollama_endpoint}
                  onChange={(e) => patch({ ollama_endpoint: e.target.value })}
                  style={inputStyle}
                />
              </Field>
              <Field label="Model">
                <input
                  type="text"
                  value={local.ollama_model}
                  onChange={(e) => patch({ ollama_model: e.target.value })}
                  placeholder="e.g. llama3.2:3b"
                  style={inputStyle}
                />
              </Field>
              <Field label="System prompt">
                <textarea
                  value={local.ollama_system_prompt}
                  onChange={(e) => patch({ ollama_system_prompt: e.target.value })}
                  rows={4}
                  style={{ ...selectStyle, resize: "vertical", fontFamily: "var(--font-sans)" }}
                />
              </Field>
            </>
          )}

          {provider === "LmStudio" && (
            <>
              <Field label="LM Studio endpoint">
                <input
                  type="text"
                  value={local.lmstudio_endpoint}
                  onChange={(e) => patch({ lmstudio_endpoint: e.target.value })}
                  style={inputStyle}
                />
              </Field>
              <Field label="Model">
                <input
                  type="text"
                  value={local.lmstudio_model}
                  onChange={(e) => patch({ lmstudio_model: e.target.value })}
                  placeholder="e.g. lmstudio-community/Meta-Llama-3-8B"
                  style={inputStyle}
                />
              </Field>
              <Field label="System prompt">
                <textarea
                  value={local.lmstudio_system_prompt}
                  onChange={(e) => patch({ lmstudio_system_prompt: e.target.value })}
                  rows={4}
                  style={{ ...selectStyle, resize: "vertical", fontFamily: "var(--font-sans)" }}
                />
              </Field>
            </>
          )}

          {provider === "OpenRouter" && (
            <>
              <Field label="API key">
                <div style={{ display: "flex", gap: 6 }}>
                  <input
                    type="password"
                    value={apiKey}
                    onChange={(e) => { setApiKey(e.target.value); setApiKeyLoaded(false); }}
                    placeholder="sk-or-..."
                    style={{ ...inputStyle, flex: 1 }}
                  />
                  <button onClick={saveApiKey} style={{ ...btnStyle, opacity: (!apiKey || apiKey === "••••••••••••••••") ? 0.5 : 1 }}>
                    Save
                  </button>
                  {apiKeyLoaded && (
                    <button onClick={deleteApiKey} style={btnStyle}>
                      Remove
                    </button>
                  )}
                </div>
                {apiKeySaveMsg && (
                  <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{apiKeySaveMsg}</span>
                )}
                <span style={{ fontSize: 11, color: "var(--text-secondary)" }}>
                  Stored securely in your OS keychain.
                </span>
              </Field>
              <Field label="Model">
                <input
                  type="text"
                  value={local.openrouter_model}
                  onChange={(e) => patch({ openrouter_model: e.target.value })}
                  placeholder="e.g. google/gemini-2.5-flash"
                  style={inputStyle}
                />
              </Field>
              <Field label="System prompt">
                <textarea
                  value={local.openrouter_system_prompt}
                  onChange={(e) => patch({ openrouter_system_prompt: e.target.value })}
                  rows={4}
                  style={{ ...selectStyle, resize: "vertical", fontFamily: "var(--font-sans)" }}
                />
              </Field>
            </>
          )}
        </>
      )}

      <SaveRow onSave={onSave} msg={saveMsg} />
    </div>
  );
}

// ── Language tab ──────────────────────────────────────────────────────────────

const WHISPER_LANGUAGES: { code: string; name: string }[] = [
  { code: "auto", name: "Auto-detect" },
  { code: "en", name: "English" },
  { code: "zh", name: "Chinese" },
  { code: "de", name: "German" },
  { code: "es", name: "Spanish" },
  { code: "ru", name: "Russian" },
  { code: "ko", name: "Korean" },
  { code: "fr", name: "French" },
  { code: "ja", name: "Japanese" },
  { code: "pt", name: "Portuguese" },
  { code: "tr", name: "Turkish" },
  { code: "pl", name: "Polish" },
  { code: "ca", name: "Catalan" },
  { code: "nl", name: "Dutch" },
  { code: "ar", name: "Arabic" },
  { code: "sv", name: "Swedish" },
  { code: "it", name: "Italian" },
  { code: "id", name: "Indonesian" },
  { code: "hi", name: "Hindi" },
  { code: "fi", name: "Finnish" },
  { code: "vi", name: "Vietnamese" },
  { code: "he", name: "Hebrew" },
  { code: "uk", name: "Ukrainian" },
  { code: "el", name: "Greek" },
  { code: "ms", name: "Malay" },
  { code: "cs", name: "Czech" },
  { code: "ro", name: "Romanian" },
  { code: "da", name: "Danish" },
  { code: "hu", name: "Hungarian" },
  { code: "ta", name: "Tamil" },
  { code: "no", name: "Norwegian" },
  { code: "th", name: "Thai" },
  { code: "ur", name: "Urdu" },
  { code: "hr", name: "Croatian" },
  { code: "bg", name: "Bulgarian" },
  { code: "lt", name: "Lithuanian" },
  { code: "la", name: "Latin" },
  { code: "mi", name: "Maori" },
  { code: "ml", name: "Malayalam" },
  { code: "cy", name: "Welsh" },
  { code: "sk", name: "Slovak" },
  { code: "te", name: "Telugu" },
  { code: "fa", name: "Persian" },
  { code: "lv", name: "Latvian" },
  { code: "bn", name: "Bengali" },
  { code: "sr", name: "Serbian" },
  { code: "az", name: "Azerbaijani" },
  { code: "sl", name: "Slovenian" },
  { code: "kn", name: "Kannada" },
  { code: "et", name: "Estonian" },
  { code: "mk", name: "Macedonian" },
  { code: "br", name: "Breton" },
  { code: "eu", name: "Basque" },
  { code: "is", name: "Icelandic" },
  { code: "hy", name: "Armenian" },
  { code: "ne", name: "Nepali" },
  { code: "mn", name: "Mongolian" },
  { code: "bs", name: "Bosnian" },
  { code: "kk", name: "Kazakh" },
  { code: "sq", name: "Albanian" },
  { code: "sw", name: "Swahili" },
  { code: "gl", name: "Galician" },
  { code: "mr", name: "Marathi" },
  { code: "pa", name: "Punjabi" },
  { code: "si", name: "Sinhala" },
  { code: "km", name: "Khmer" },
  { code: "sn", name: "Shona" },
  { code: "yo", name: "Yoruba" },
  { code: "so", name: "Somali" },
  { code: "af", name: "Afrikaans" },
  { code: "oc", name: "Occitan" },
  { code: "ka", name: "Georgian" },
  { code: "be", name: "Belarusian" },
  { code: "tg", name: "Tajik" },
  { code: "sd", name: "Sindhi" },
  { code: "gu", name: "Gujarati" },
  { code: "am", name: "Amharic" },
  { code: "yi", name: "Yiddish" },
  { code: "lo", name: "Lao" },
  { code: "uz", name: "Uzbek" },
  { code: "fo", name: "Faroese" },
  { code: "ht", name: "Haitian Creole" },
  { code: "ps", name: "Pashto" },
  { code: "tk", name: "Turkmen" },
  { code: "nn", name: "Nynorsk" },
  { code: "mt", name: "Maltese" },
  { code: "sa", name: "Sanskrit" },
  { code: "lb", name: "Luxembourgish" },
  { code: "my", name: "Myanmar" },
  { code: "bo", name: "Tibetan" },
  { code: "tl", name: "Tagalog" },
  { code: "mg", name: "Malagasy" },
  { code: "as", name: "Assamese" },
  { code: "tt", name: "Tatar" },
  { code: "haw", name: "Hawaiian" },
  { code: "ln", name: "Lingala" },
  { code: "ha", name: "Hausa" },
  { code: "ba", name: "Bashkir" },
  { code: "jw", name: "Javanese" },
  { code: "su", name: "Sundanese" },
];

// ── Hotkey capture ────────────────────────────────────────────────────────────

const MODIFIER_CODES = new Set(["AltLeft", "AltRight", "ControlLeft", "ControlRight", "MetaLeft", "MetaRight", "ShiftLeft", "ShiftRight"]);

function codeToKeyName(code: string): string | null {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("F") && /^F\d+$/.test(code)) return code;
  const map: Record<string, string> = {
    Space: "Space", Enter: "Enter", Escape: "Escape", Tab: "Tab",
    Backspace: "Backspace", Delete: "Delete", Insert: "Insert",
    Home: "Home", End: "End", PageUp: "PageUp", PageDown: "PageDown",
    ArrowUp: "ArrowUp", ArrowDown: "ArrowDown", ArrowLeft: "ArrowLeft", ArrowRight: "ArrowRight",
    Minus: "-", Equal: "=", BracketLeft: "[", BracketRight: "]",
    Backslash: "\\", Semicolon: ";", Quote: "'", Comma: ",", Period: ".", Slash: "/",
    Backquote: "`",
  };
  return map[code] ?? null;
}

function HotkeyCapture({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [capturing, setCapturing] = useState(false);
  const [preview, setPreview] = useState("");
  const ref = useRef<HTMLDivElement>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    if (!capturing) return;
    ref.current?.focus();

    function onKeyDown(e: KeyboardEvent) {
      e.preventDefault();
      e.stopPropagation();

      if (e.code === "Escape") {
        setCapturing(false);
        setPreview("");
        return;
      }

      const parts: string[] = [];
      if (e.ctrlKey)  parts.push("Ctrl");
      if (e.altKey)   parts.push("Alt");
      if (e.shiftKey) parts.push("Shift");
      if (e.metaKey)  parts.push("Super");

      if (MODIFIER_CODES.has(e.code)) {
        setPreview(parts.join("+") || "…");
        return;
      }

      const key = codeToKeyName(e.code);
      if (!key) return;
      parts.push(key);

      const combo = parts.join("+");
      setPreview(combo);
      onChangeRef.current(combo);
      setCapturing(false);
    }

    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, [capturing]);

  return (
    <div
      ref={ref}
      tabIndex={-1}
      onClick={() => { setCapturing(true); setPreview(""); }}
      style={{
        ...inputStyle,
        cursor: "pointer",
        userSelect: "none",
        outline: capturing ? "2px solid var(--text-primary)" : undefined,
        color: capturing ? "var(--text-secondary)" : "var(--text-primary)",
      }}
    >
      {capturing
        ? (preview || "Press keys…")
        : (value || "Click to set")}
    </div>
  );
}

// ── Hotkeys tab ───────────────────────────────────────────────────────────────

function HotkeysTab({
  local,
  patch,
  onSave,
  saveMsg,
}: {
  local: Settings;
  patch: (p: Partial<Settings>) => void;
  onSave: () => void;
  saveMsg: string;
}) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 20 }}>
      <Field label="Push to Talk">
        <HotkeyCapture value={local.hotkey_ptt} onChange={(v) => patch({ hotkey_ptt: v })} />
      </Field>
      <Field label="Toggle">
        <HotkeyCapture value={local.hotkey_toggle} onChange={(v) => patch({ hotkey_toggle: v })} />
      </Field>
      <SaveRow onSave={onSave} msg={saveMsg} />
    </div>
  );
}

// ── Shared UI helpers ─────────────────────────────────────────────────────────

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
      <label style={{ fontSize: 12, color: "var(--text-secondary)", fontWeight: 500 }}>{label}</label>
      {children}
    </div>
  );
}

function SaveRow({ onSave, msg }: { onSave: () => void; msg: string }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12, marginTop: 4 }}>
      <button onClick={onSave} style={{ ...btnStyle, display: "flex", alignItems: "center", gap: 4 }}>
        <Save size={14} strokeWidth={1.5} /> Save
      </button>
      {msg && <span style={{ fontSize: 12, color: "var(--text-secondary)" }}>{msg}</span>}
    </div>
  );
}

const selectStyle: React.CSSProperties = {
  fontSize: 13,
  padding: "6px 10px",
  borderRadius: 6,
  border: "1px solid var(--border)",
  background: "var(--bg-surface)",
  color: "var(--text-primary)",
  width: "100%",
};

const inputStyle: React.CSSProperties = {
  ...selectStyle,
  fontFamily: "var(--font-mono)",
};

const btnStyle: React.CSSProperties = {
  fontSize: 12,
  padding: "5px 12px",
  borderRadius: 6,
  border: "1px solid var(--border)",
  background: "var(--bg-surface)",
  color: "var(--text-primary)",
  cursor: "pointer",
};
