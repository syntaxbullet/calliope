import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "../../store";
import type { AppState, Settings } from "../../types";

const BAR_COUNT = 24;

function label(state: AppState): string {
  switch (state.type) {
    case "Recording":      return "Listening...";
    case "Transcribing":   return "Transcribing...";
    case "PostProcessing": return "Processing...";
    case "Injecting":      return "Injecting...";
    default:               return "";
  }
}

export function OverlayView() {
  const { appState, setAppState } = useAppStore();
  const [level, setLevel] = useState(0);
  const [threshold, setThreshold] = useState(0.05);

  const [visible, setVisible] = useState(false);

  useEffect(() => {
    invoke<Settings>("get_settings").then((s) => setThreshold(s.silence_threshold)).catch(() => {});
  }, []);

  useEffect(() => {
    document.documentElement.classList.add("overlay-window");

    const onVisibility = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onVisibility);
    setVisible(document.visibilityState === "visible");

    return () => document.removeEventListener("visibilitychange", onVisibility);
  }, []);

  // Only poll when the overlay window is visible
  useEffect(() => {
    if (!visible) return;

    const stateInterval = setInterval(() => {
      invoke<AppState>("get_app_state").then(setAppState).catch(() => {});
    }, 200);

    const levelInterval = setInterval(() => {
      invoke<number>("get_audio_level").then(setLevel).catch(() => {});
    }, 50);

    // Sync state immediately when becoming visible
    invoke<AppState>("get_app_state").then(setAppState).catch(() => {});

    return () => {
      clearInterval(stateInterval);
      clearInterval(levelInterval);
    };
  }, [visible]);
  const isRecording = appState.type === "Recording";
  const isSilence = level < threshold;
  const barColor = isRecording
    ? isSilence ? "#ef4444" : "#22c55e"
    : "var(--text-primary)";

  return (
    <div
      style={{
        width: "100vw",
        height: "100vh",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background: "transparent",
      }}
    >
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "10px 20px",
          borderRadius: 9999,
          background: "color-mix(in srgb, var(--bg-surface) 85%, transparent)",
          backdropFilter: "blur(12px)",
          WebkitBackdropFilter: "blur(12px)",
          border: "1px solid var(--border)",
        }}
      >
        {/* Waveform */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            gap: 2,
            height: 28,
          }}
        >
          {Array.from({ length: BAR_COUNT }).map((_, i) => {
            const spread = Math.sin((i / (BAR_COUNT - 1)) * Math.PI);
            // Amplify and apply log scaling so small RMS values still produce visible bars
            const amp = Math.min(Math.sqrt(level) * 3, 1.0);
            const height = isRecording
              ? Math.max(4, amp * 24 * (0.3 + 0.7 * spread) + 4)
              : 4;
            return (
              <div
                key={i}
                style={{
                  width: 2.5,
                  height,
                  borderRadius: 1.5,
                  background: barColor,
                  transition: "height 0.05s ease-out, background 0.15s ease-out",
                }}
              />
            );
          })}
        </div>

        {/* RMS debug + state label */}
        <span
          style={{
            fontSize: 12,
            color: "var(--text-secondary)",
            whiteSpace: "nowrap",
            userSelect: "none",
            fontFamily: "var(--font-mono)",
          }}
        >
          {isRecording && (
            <span style={{ color: barColor, marginRight: 8 }}>
              {level.toFixed(3)}
            </span>
          )}
          {label(appState)}
        </span>
      </div>
    </div>
  );
}
