import { useAppStore } from "../../store";
import type { AppState } from "../../types";

const BAR_COUNT = 20;

function label(state: AppState): string {
  switch (state.type) {
    case "Recording":      return "Listening…";
    case "Transcribing":   return "Transcribing…";
    case "PostProcessing": return "Processing…";
    case "Injecting":      return "Injecting…";
    case "Error":          return `Error: ${state.message}`;
    default:               return "Ready";
  }
}

function isError(state: AppState): boolean {
  return state.type === "Error";
}

export function StatusIndicator() {
  const { appState, audioLevel } = useAppStore();
  const isRecording = appState.type === "Recording";
  const err = isError(appState);

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        gap: 12,
      }}
    >
      {/* Waveform bars — only animated while recording */}
      <div
        style={{
          display: "flex",
          alignItems: "flex-end",
          gap: 2,
          height: 40,
          opacity: isRecording ? 1 : 0.25,
          transition: "opacity 0.2s",
        }}
      >
        {Array.from({ length: BAR_COUNT }).map((_, i) => {
          // Give bars a slight spread so they don't all move identically
          const spread = Math.sin((i / BAR_COUNT) * Math.PI);
          const height = isRecording
            ? Math.max(4, audioLevel * 40 * spread + 4)
            : 4;
          return (
            <div
              key={i}
              style={{
                width: 3,
                height,
                borderRadius: 2,
                background: err ? "var(--text-secondary)" : "var(--text-primary)",
                transition: "height 0.05s ease-out",
              }}
            />
          );
        })}
      </div>

      {/* State label */}
      <p
        className="text-body"
        style={{
          color: err ? "var(--text-secondary)" : "var(--text-primary)",
          margin: 0,
        }}
      >
        {label(appState)}
      </p>
    </div>
  );
}
