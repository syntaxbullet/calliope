import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "./store";
import type { AppState, AudioLevelEvent, Settings } from "./types";
import { Settings as SettingsIcon, X } from "lucide-react";
import { StatusIndicator } from "./components/StatusIndicator";
import { SettingsPanel } from "./components/Settings";
import { Toast } from "./components/Toast";
import { OverlayView } from "./components/Overlay";
import { Onboarding } from "./components/Onboarding";

function PopoverView() {
  const { settings, setSettings, accelerationBackend, onboardingComplete } = useAppStore();

  const openSettings = async () => {
    const { Window } = await import("@tauri-apps/api/window");
    const win = await Window.getByLabel("settings");
    if (win) {
      await win.show();
      await win.setFocus();
    }
  };

  const closePopover = async () => {
    const win = getCurrentWindow();
    await win.hide();
  };

  if (!onboardingComplete) {
    return <Onboarding />;
  }

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        justifyContent: "center",
        height: "100vh",
        background: "var(--bg-base)",
        color: "var(--text-primary)",
        position: "relative",
      }}
    >
      <button
        onClick={closePopover}
        style={{
          position: "absolute",
          top: 10,
          right: 10,
          background: "none",
          border: "none",
          cursor: "pointer",
          color: "var(--text-secondary)",
          fontSize: 16,
          lineHeight: 1,
          padding: "2px 6px",
          borderRadius: 4,
        }}
        aria-label="Close"
      >
        <X size={16} strokeWidth={1.5} />
      </button>

      <StatusIndicator />

      <div
        style={{
          position: "absolute",
          bottom: 16,
          left: 16,
          display: "flex",
          flexDirection: "column",
          gap: 4,
        }}
      >
        <span style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "var(--font-mono)" }}>
          Model: {settings.active_model ?? "none"}
        </span>
        <span style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "var(--font-mono)" }}>
          PTT: {settings.hotkey_ptt}
        </span>
        <span style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "var(--font-mono)" }}>
          Toggle: {settings.hotkey_toggle}
        </span>
        <span style={{ fontSize: 11, color: "var(--text-secondary)", fontFamily: "var(--font-mono)" }}>
          Hardware Acceleration: {settings.use_gpu && accelerationBackend !== "CPU" ? accelerationBackend : "CPU (no acceleration)"}
        </span>
        <div
          onClick={() => {
            const newMode = settings.injection_mode === "Clipboard" ? "Character" : "Clipboard";
            const updated = { ...settings, injection_mode: newMode };
            setSettings({ injection_mode: newMode });
            invoke("save_settings", { settings: updated }).catch(console.error);
          }}
          style={{ display: "flex", alignItems: "center", gap: 4, cursor: "pointer", fontSize: 11, fontFamily: "var(--font-mono)" }}
        >
          <span style={{ color: settings.injection_mode === "Clipboard" ? "var(--text-primary)" : "var(--text-secondary)", transition: "color 0.2s" }}>
            Clipboard
          </span>
          <span style={{ color: "var(--border)" }}>/</span>
          <span style={{ color: settings.injection_mode === "Character" ? "var(--text-primary)" : "var(--text-secondary)", transition: "color 0.2s" }}>
            Character
          </span>
        </div>
      </div>

      <button
        onClick={openSettings}
        style={{
          position: "absolute",
          bottom: 16,
          right: 16,
          background: "none",
          border: "none",
          cursor: "pointer",
          color: "var(--text-secondary)",
          display: "flex",
          alignItems: "center",
          padding: 6,
          borderRadius: 6,
        }}
        aria-label="Open settings"
      >
        <SettingsIcon size={16} strokeWidth={1.5} />
      </button>
      <Toast />
    </div>
  );
}

function App() {
  const { setAppState, setAudioLevel, settings, setSettings, setAccelerationBackend } =
    useAppStore();
  const [windowLabel, setWindowLabel] = useState<string | null>(null);

  useEffect(() => {
    setWindowLabel(getCurrentWindow().label);
  }, []);

  useEffect(() => {
    const unlistenState = listen<AppState>("app-state-changed", (event) => {
      setAppState(event.payload);
    });

    const unlistenAudio = listen<AudioLevelEvent>("audio-level", (event) => {
      setAudioLevel(event.payload.rms);
    });

    invoke<AppState>("get_app_state")
      .then((s) => setAppState(s))
      .catch(console.error);

    invoke<typeof settings>("get_settings")
      .then((s) => setSettings(s))
      .catch(console.error);

    invoke<string>("get_acceleration_backend")
      .then((b) => setAccelerationBackend(b))
      .catch(console.error);

    const unlistenSettings = listen<Settings>("settings-changed", (event) => {
      setSettings(event.payload);
    });

    const onVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        invoke<typeof settings>("get_settings")
          .then((s) => setSettings(s))
          .catch(console.error);
      }
    };
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      unlistenState.then((f) => f());
      unlistenAudio.then((f) => f());
      unlistenSettings.then((f) => f());
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, []);

  // Apply theme class to document root
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("theme-light", "theme-dark");
    if (settings.theme === "Light") {
      root.classList.add("theme-light");
    } else if (settings.theme === "Dark") {
      root.classList.add("theme-dark");
    }
    // "System" — no class, prefers-color-scheme takes over
  }, [settings.theme]);

  if (windowLabel === null) return null;

  if (windowLabel === "settings") {
    return <SettingsPanel />;
  }

  if (windowLabel === "overlay") {
    return <OverlayView />;
  }

  return <PopoverView />;
}

export default App;
