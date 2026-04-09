'use client';

import { useChat as useBaseChat } from '@ai-sdk/react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/**
 * Wraps @ai-sdk/react useChat with a custom fetch that routes all LLM calls
 * through Rust (via Tauri IPC), bypassing webview CSP restrictions on fetch().
 * Tokens stream back as Tauri events and are piped into a ReadableStream in
 * the Vercel AI SDK data-stream format so useBaseChat can consume them normally.
 */
export const useChat = () => {
  return useBaseChat({
    id: 'editor',
    api: '/api/ai/command', // never reached — overridden below
    fetch: async (_input, init) => {
      const body = JSON.parse((init?.body as string) ?? '{}');
      const messages: { role: string; content: string }[] = body.messages ?? [];
      const streamId = crypto.randomUUID();
      const enc = new TextEncoder();

      const stream = new ReadableStream<Uint8Array>({
        start(controller) {
          const unlisteners: Array<() => void> = [];

          const cleanup = () => unlisteners.forEach((u) => u());

          // Register event listeners first, then invoke the command so we
          // never miss the first chunk.
          Promise.all([
            listen<string>(`llm-chunk:${streamId}`, (e) => {
              controller.enqueue(enc.encode(`0:${JSON.stringify(e.payload)}\n`));
            }),
            listen<null>(`llm-done:${streamId}`, () => {
              cleanup();
              controller.close();
            }),
            listen<string>(`llm-error:${streamId}`, (e) => {
              cleanup();
              controller.error(new Error(e.payload ?? 'LLM error'));
            }),
          ]).then((unls) => {
            unlisteners.push(...unls);

            // Handle abort signal
            const signal = (init as RequestInit | undefined)?.signal;
            if (signal) {
              signal.addEventListener('abort', () => {
                cleanup();
                controller.close();
              }, { once: true });
            }

            // Kick off the Rust-side streaming call
            invoke('llm_stream', { messages, streamId }).catch((err: unknown) => {
              cleanup();
              controller.error(new Error(String(err)));
            });
          }).catch((err: unknown) => {
            controller.error(new Error(String(err)));
          });
        },
      });

      return new Response(stream, {
        headers: { 'Content-Type': 'text/plain; charset=utf-8' },
      });
    },
  });
};
