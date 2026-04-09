/**
 * Module-level Zustand store for AI editor config.
 * Stored outside React so `copilot-plugins.tsx` (a plugin constant, not a component)
 * can read the current values synchronously via `useAiChatConfig.getState()`.
 * Updated by SettingsProvider on mount and on every save.
 */
import { create } from 'zustand';

interface AiChatConfigState {
  endpoint: string;
  apiKey: string;
  writerModel: string;
  setConfig: (cfg: { endpoint: string; apiKey: string; writerModel: string }) => void;
}

export const useAiChatConfig = create<AiChatConfigState>((set) => ({
  endpoint: '',
  apiKey: '',
  writerModel: 'gpt-4o-mini',
  setConfig: (cfg) => set(cfg),
}));
