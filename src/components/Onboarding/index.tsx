import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "../../store";
import type { ModelInfo } from "../../types";

interface DownloadProgressEvent {
  name: string;
  bytes_downloaded: number;
  total_bytes: number;
}

function StepDots({ current, total }: { current: number; total: number }) {
  return (
    <div style={{ display: "flex", gap: 6, justifyContent: "center", marginBottom: 24 }}>
      {Array.from({ length: total }, (_, i) => (
        <div
          key={i}
          style={{
            width: 6,
            height: 6,
            borderRadius: "50%",
            background: i === current ? "var(--text-primary)" : "var(--border)",
            transition: "background 0.2s",
          }}
        />
      ))}
    </div>
  );
}

function WelcomeStep({ onNext }: { onNext: () => void }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 16, textAlign: "center" }}>
      <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Welcome to Calliope</h2>
      <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5, margin: 0, maxWidth: 220 }}>
        Speak naturally. Your words appear wherever you type.
      </p>
      <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5, margin: 0 }}>
        Everything stays on your device.
      </p>
      <button onClick={onNext} style={primaryButtonStyle}>
        Get Started
      </button>
    </div>
  );
}

function PermissionsStep({ onNext }: { onNext: () => void }) {
  const [granted, setGranted] = useState(false);
  const [checking, setChecking] = useState(true);

  useEffect(() => {
    invoke<boolean>("check_accessibility").then((ok) => {
      setGranted(ok);
      setChecking(false);
      if (ok) onNext();
    });
  }, []);

  const handleGrant = async () => {
    const ok = await invoke<boolean>("request_accessibility");
    setGranted(ok);
    if (ok) {
      setTimeout(onNext, 600);
    }
  };

  // Poll after requesting — macOS requires user action in System Settings
  useEffect(() => {
    if (granted || checking) return;
    const interval = setInterval(async () => {
      const ok = await invoke<boolean>("check_accessibility");
      if (ok) {
        setGranted(true);
        clearInterval(interval);
        setTimeout(onNext, 600);
      }
    }, 1500);
    return () => clearInterval(interval);
  }, [granted, checking]);

  if (checking) return null;

  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 16, textAlign: "center" }}>
      <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>Accessibility Access</h2>
      <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5, margin: 0, maxWidth: 220 }}>
        Calliope needs Accessibility access to type text into other apps.
      </p>
      {granted ? (
        <div style={{ display: "flex", alignItems: "center", gap: 6, color: "var(--text-primary)", fontSize: 13 }}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <polyline points="20 6 9 17 4 12" />
          </svg>
          Granted
        </div>
      ) : (
        <button onClick={handleGrant} style={primaryButtonStyle}>
          Grant Access
        </button>
      )}
      {!granted && (
        <button onClick={onNext} style={linkButtonStyle}>
          Skip
        </button>
      )}
    </div>
  );
}

function ModelStep({ onNext }: { onNext: () => void }) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [downloading, setDownloading] = useState<Record<string, number>>({});
  const [hasDownloaded, setHasDownloaded] = useState(false);

  useEffect(() => {
    invoke<ModelInfo[]>("list_models").then((list) => {
      setModels(list);
      // Default selection
      const turbo = list.find((m) => m.name === "large-v3-turbo");
      const first = turbo ?? list[0];
      if (first) setSelected(first.name);
      // Check if any model is already downloaded
      if (list.some((m) => m.downloaded)) setHasDownloaded(true);
    });
  }, []);

  useEffect(() => {
    const unlisten = listen<DownloadProgressEvent>("download-progress", (e) => {
      const { name, bytes_downloaded, total_bytes } = e.payload;
      const pct = total_bytes > 0 ? Math.round((bytes_downloaded / total_bytes) * 100) : 0;
      setDownloading((prev) => ({ ...prev, [name]: pct }));
    });
    return () => { unlisten.then((f) => f()); };
  }, []);

  const handleDownload = async () => {
    if (!selected) return;
    setDownloading((prev) => ({ ...prev, [selected]: 0 }));
    try {
      await invoke("download_model", { name: selected });
      await invoke("set_active_model", { name: selected });
      const updated = await invoke<ModelInfo[]>("list_models");
      setModels(updated);
      setHasDownloaded(true);
      setDownloading((prev) => {
        const next = { ...prev };
        delete next[selected];
        return next;
      });
    } catch (e) {
      console.error("Download failed:", e);
      setDownloading((prev) => {
        const next = { ...prev };
        delete next[selected];
        return next;
      });
    }
  };

  const isDownloading = selected !== null && downloading[selected] !== undefined;
  const selectedModel = models.find((m) => m.name === selected);
  const canContinue = hasDownloaded;

  function formatSize(bytes: number) {
    if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
    return `${Math.round(bytes / 1e6)} MB`;
  }

  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12, width: "100%" }}>
      <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0, textAlign: "center" }}>Download a Model</h2>
      <div style={{ display: "flex", flexDirection: "column", gap: 6, maxHeight: 180, overflowY: "auto", padding: "0 4px" }}>
        {models.map((m) => {
          const pct = downloading[m.name];
          const isActive = m.name === selected;
          return (
            <label
              key={m.name}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 8,
                padding: "8px 10px",
                borderRadius: 8,
                background: isActive ? "var(--bg-elevated)" : "transparent",
                cursor: "pointer",
                fontSize: 13,
                transition: "background 0.15s",
              }}
            >
              <input
                type="radio"
                name="model"
                checked={isActive}
                onChange={() => setSelected(m.name)}
                style={{ accentColor: "var(--text-primary)" }}
              />
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                  <span style={{ fontWeight: 500 }}>{m.name}</span>
                  <span style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "var(--font-mono)" }}>
                    {formatSize(m.size_bytes)}
                  </span>
                  {m.downloaded && (
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" style={{ color: "var(--text-secondary)" }}>
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  )}
                </div>
                {pct !== undefined && (
                  <div style={{ marginTop: 4, height: 4, borderRadius: 2, background: "var(--border)", overflow: "hidden" }}>
                    <div style={{ height: "100%", width: `${pct}%`, background: "var(--text-primary)", borderRadius: 2, transition: "width 0.3s" }} />
                  </div>
                )}
              </div>
            </label>
          );
        })}
      </div>
      {!canContinue && (
        <button
          onClick={handleDownload}
          disabled={!selected || isDownloading || (selectedModel?.downloaded ?? false)}
          style={{
            ...primaryButtonStyle,
            opacity: !selected || isDownloading || (selectedModel?.downloaded ?? false) ? 0.5 : 1,
          }}
        >
          {isDownloading ? `Downloading... ${downloading[selected!]}%` : selectedModel?.downloaded ? "Already Downloaded" : "Download"}
        </button>
      )}
      {canContinue && (
        <button onClick={onNext} style={primaryButtonStyle}>
          Continue
        </button>
      )}
    </div>
  );
}

function SuccessStep({ onDone }: { onDone: () => void }) {
  const { settings } = useAppStore();
  const hotkey = settings.recording_mode === "PushToTalk" ? settings.hotkey_ptt : settings.hotkey_toggle;
  const action = settings.recording_mode === "PushToTalk" ? "Hold" : "Press";
  const suffix = settings.recording_mode === "PushToTalk" ? "to start speaking. Release to transcribe." : "to start speaking. Press again to stop.";

  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 16, textAlign: "center" }}>
      <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>You're ready.</h2>
      <p style={{ fontSize: 13, color: "var(--text-secondary)", lineHeight: 1.5, margin: 0, maxWidth: 220 }}>
        {action}{" "}
        <span style={{ fontFamily: "var(--font-mono)", background: "var(--bg-elevated)", padding: "2px 6px", borderRadius: 4, fontSize: 12 }}>
          {hotkey}
        </span>{" "}
        {suffix}
      </p>
      <p style={{ fontSize: 13, color: "var(--text-secondary)", margin: 0 }}>
        Open any text field and try it now.
      </p>
      <button onClick={onDone} style={primaryButtonStyle}>
        Done
      </button>
    </div>
  );
}

const primaryButtonStyle: React.CSSProperties = {
  padding: "8px 24px",
  borderRadius: 8,
  border: "none",
  background: "var(--text-primary)",
  color: "var(--bg-base)",
  fontSize: 13,
  fontWeight: 500,
  cursor: "pointer",
  fontFamily: "inherit",
  transition: "opacity 0.15s",
};

const linkButtonStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: "var(--text-secondary)",
  fontSize: 12,
  cursor: "pointer",
  textDecoration: "underline",
  fontFamily: "inherit",
};

export function Onboarding() {
  const { settings, setSettings, setOnboardingComplete } = useAppStore();
  const [step, setStep] = useState(0);
  const isMac = navigator.userAgent.includes("Mac");

  // Total steps: skip permissions step on non-macOS
  const totalSteps = isMac ? 4 : 3;

  const next = () => setStep((s) => s + 1);

  const handleDone = async () => {
    const updated = { ...settings, onboarding_complete: true };
    try {
      await invoke("save_settings", { settings: updated });
      setSettings({ onboarding_complete: true });
      setOnboardingComplete(true);
    } catch (e) {
      console.error("Failed to save onboarding state:", e);
    }
  };

  // Map logical step to component, skipping permissions on non-Mac
  const steps = isMac
    ? [
        <WelcomeStep key="welcome" onNext={next} />,
        <PermissionsStep key="perms" onNext={next} />,
        <ModelStep key="model" onNext={next} />,
        <SuccessStep key="success" onDone={handleDone} />,
      ]
    : [
        <WelcomeStep key="welcome" onNext={next} />,
        <ModelStep key="model" onNext={next} />,
        <SuccessStep key="success" onDone={handleDone} />,
      ];

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        justifyContent: "center",
        alignItems: "center",
        height: "100vh",
        background: "var(--bg-base)",
        color: "var(--text-primary)",
        padding: 24,
        boxSizing: "border-box",
      }}
    >
      <StepDots current={step} total={totalSteps} />
      <div style={{ width: "100%", maxWidth: 240 }}>
        {steps[step]}
      </div>
    </div>
  );
}
