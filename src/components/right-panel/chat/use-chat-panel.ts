import { useChat } from "@ai-sdk/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/**
 * Chat panel variant of the LLM hook — shares the same Rust llm_stream pipeline
 * as the editor AI chat, but with a separate conversation id ("panel").
 */
export function useBaseChat() {
  return useChat({
    id: "panel",
    api: "/api/ai/chat",
    fetch: async (_input, init) => {
      const body = JSON.parse((init?.body as string) ?? "{}");
      const messages: { role: string; content: string }[] = body.messages ?? [];
      const streamId = crypto.randomUUID();
      const enc = new TextEncoder();

      const stream = new ReadableStream<Uint8Array>({
        start(controller) {
          const unlisteners: Array<() => void> = [];
          const cleanup = () => unlisteners.forEach((u) => u());

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
              controller.error(new Error(e.payload ?? "LLM error"));
            }),
          ]).then((unls) => {
            unlisteners.push(...unls);

            const signal = (init as RequestInit | undefined)?.signal;
            if (signal) {
              signal.addEventListener("abort", () => {
                cleanup();
                controller.close();
              }, { once: true });
            }

            invoke("llm_stream", { messages, streamId }).catch((err: unknown) => {
              cleanup();
              controller.error(new Error(String(err)));
            });
          }).catch((err: unknown) => {
            controller.error(new Error(String(err)));
          });
        },
      });

      return new Response(stream, {
        headers: { "Content-Type": "text/plain; charset=utf-8" },
      });
    },
  });
}
