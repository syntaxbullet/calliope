import { create } from "zustand";
import type { AppState, Settings, ModelInfo, AudioDevice } from "../types";
import { DEFAULT_SETTINGS } from "../types";

interface AppStore {
  // App state machine
  appState: AppState;
  setAppState: (state: AppState) => void;

  // Settings
  settings: Settings;
  setSettings: (settings: Partial<Settings>) => void;

  // Models
  models: ModelInfo[];
  setModels: (models: ModelInfo[]) => void;

  // Audio
  audioDevices: AudioDevice[];
  setAudioDevices: (devices: AudioDevice[]) => void;
  audioLevel: number;
  setAudioLevel: (level: number) => void;

  // Hardware
  accelerationBackend: string;
  setAccelerationBackend: (backend: string) => void;

  // UI
  settingsOpen: boolean;
  setSettingsOpen: (open: boolean) => void;
  onboardingComplete: boolean;
  setOnboardingComplete: (complete: boolean) => void;
}

export const useAppStore = create<AppStore>((set) => ({
  appState: { type: "Idle" },
  setAppState: (appState) => set({ appState }),

  settings: DEFAULT_SETTINGS,
  setSettings: (partial) =>
    set((s) => {
      const merged = { ...s.settings, ...partial };
      return {
        settings: merged,
        onboardingComplete: merged.onboarding_complete,
      };
    }),

  models: [],
  setModels: (models) => set({ models }),

  audioDevices: [],
  setAudioDevices: (audioDevices) => set({ audioDevices }),
  audioLevel: 0,
  setAudioLevel: (audioLevel) => set({ audioLevel }),

  accelerationBackend: "CPU",
  setAccelerationBackend: (accelerationBackend) => set({ accelerationBackend }),

  settingsOpen: false,
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),

  onboardingComplete: false,
  setOnboardingComplete: (onboardingComplete) => set({ onboardingComplete }),
}));
