'use client';

import type { TElement } from '@udecode/plate';

import { CopilotPlugin } from '@udecode/plate-ai/react';
import { serializeMd, stripMarkdown } from '@udecode/plate-markdown';
import { invoke } from '@tauri-apps/api/core';

import { GhostText } from '@/components/plate-ui/ghost-text';

import { markdownPlugin } from './markdown-plugin';

export const copilotPlugins = [
  markdownPlugin,
  CopilotPlugin.configure(({ api }) => ({
    options: {
      completeOptions: {
        api: '/api/ai/copilot', // overridden by fetch below
        onFinish: (_, completion) => {
          if (completion === '0') return;
          api.copilot.setBlockSuggestion({
            text: stripMarkdown(completion),
          });
        },
        fetch: async (_input, init) => {
          const body = JSON.parse((init?.body as string) ?? '{}');
          const prompt: string = body.prompt ?? '';

          try {
            const text = await invoke<string>('copilot_complete', { prompt });
            return new Response(text, { headers: { 'Content-Type': 'text/plain' } });
          } catch {
            return new Response('0', { headers: { 'Content-Type': 'text/plain' } });
          }
        },
      },
      debounceDelay: 500,
      renderGhostText: GhostText,
      getPrompt: ({ editor }) => {
        const contextEntry = editor.api.block({ highest: true });

        if (!contextEntry) return '';

        const prompt = serializeMd(editor, {
          value: [contextEntry[0] as TElement],
        });

        return prompt;
      },
    },
  })),
] as const;
