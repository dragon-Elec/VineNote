import { create } from 'zustand';

export type AppMode = 'NoteMode' | 'StandaloneEdit';

export interface IAppModeState {
  appMode: AppMode;
  setAppMode: (mode: AppMode) => void;
}

export const useAppMode = create<IAppModeState>(set => ({
  appMode: 'NoteMode',
  setAppMode: (mode: AppMode) => set({ appMode: mode }),
}));
