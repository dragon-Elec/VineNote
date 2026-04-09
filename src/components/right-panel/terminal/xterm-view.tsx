import { useEffect, useRef, useCallback } from "react";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import "@xterm/xterm/css/xterm.css";
import { useTerminalStore } from "./use-terminal";

interface XtermViewProps {
  tabId: string;
  isVisible: boolean;
}

function buildTheme(): import("@xterm/xterm").ITheme {
  // Detect dark mode
  const isDark = document.documentElement.classList.contains("dark") ||
    window.matchMedia("(prefers-color-scheme: dark)").matches;

  return isDark
    ? {
        background: "#111113",
        foreground: "#e5e7eb",
        cursor: "#4ade80",
        cursorAccent: "#0f1117",
        selectionBackground: "rgba(74, 222, 128, 0.18)",
        black: "#1c1c1e",
        red: "#f87171",
        green: "#4ade80",
        yellow: "#facc15",
        blue: "#60a5fa",
        magenta: "#c084fc",
        cyan: "#34d399",
        white: "#e5e7eb",
        brightBlack: "#374151",
        brightRed: "#fca5a5",
        brightGreen: "#86efac",
        brightYellow: "#fde68a",
        brightBlue: "#93c5fd",
        brightMagenta: "#d8b4fe",
        brightCyan: "#6ee7b7",
        brightWhite: "#f9fafb",
      }
    : {
        background: "#f5f5f6",
        foreground: "#1c1c1e",
        cursor: "#16a34a",
        cursorAccent: "#f5f5f6",
        selectionBackground: "rgba(22, 163, 74, 0.12)",
        black: "#1c1c1e",
        red: "#dc2626",
        green: "#16a34a",
        yellow: "#ca8a04",
        blue: "#2563eb",
        magenta: "#7c3aed",
        cyan: "#0891b2",
        white: "#6b7280",
        brightBlack: "#374151",
        brightRed: "#ef4444",
        brightGreen: "#22c55e",
        brightYellow: "#eab308",
        brightBlue: "#3b82f6",
        brightMagenta: "#8b5cf6",
        brightCyan: "#06b6d4",
        brightWhite: "#111827",
      };
}

export function XtermView({ tabId, isVisible }: XtermViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const termRef = useRef<Terminal | null>(null);
  const fitAddonRef = useRef<FitAddon | null>(null);
  const unlistenDataRef = useRef<(() => void) | null>(null);
  const unlistenExitRef = useRef<(() => void) | null>(null);
  const markActivity = useTerminalStore((s) => s.markActivity);

  const fit = useCallback(() => {
    if (!fitAddonRef.current || !termRef.current) return;
    try {
      fitAddonRef.current.fit();
      const { rows, cols } = termRef.current;
      invoke("resize_terminal", { id: tabId, rows, cols }).catch(() => {});
    } catch {
      // ignore if terminal not ready
    }
  }, [tabId]);

  // Mount xterm once
  useEffect(() => {
    if (!containerRef.current || termRef.current) return;

    const term = new Terminal({
      fontFamily: '"JetBrains Mono", ui-monospace, monospace',
      fontSize: 13,
      lineHeight: 1.4,
      cursorBlink: true,
      allowProposedApi: true,
      theme: buildTheme(),
      scrollback: 5000,
    });

    const fitAddon = new FitAddon();
    const webLinksAddon = new WebLinksAddon();

    term.loadAddon(fitAddon);
    term.loadAddon(webLinksAddon);
    term.open(containerRef.current);

    termRef.current = term;
    fitAddonRef.current = fitAddon;

    // Initial fit
    setTimeout(fit, 50);

    // User input → Rust PTY write
    // Note: avoid btoa(String.fromCharCode(...bytes)) spread — it throws
    // "Maximum call stack size exceeded" for large inputs. Use a loop instead.
    term.onData((data) => {
      const bytes = new TextEncoder().encode(data);
      let binaryStr = "";
      for (let i = 0; i < bytes.length; i++) {
        binaryStr += String.fromCharCode(bytes[i]);
      }
      invoke("write_terminal", { id: tabId, data: btoa(binaryStr) }).catch(() => {});
    });

    // PTY output → xterm write (base64-encoded)
    // Use refs for cleanup so listener is correctly unregistered even if the
    // component unmounts before the async listen() promises resolve.
    listen<string>(`terminal-data:${tabId}`, (e) => {
      try {
        const raw = atob(e.payload);
        const bytes = Uint8Array.from(raw, (c) => c.charCodeAt(0));
        term.write(bytes);
        markActivity(tabId);
      } catch {
        term.write(e.payload);
      }
    }).then((u) => {
      unlistenDataRef.current = u;
    });

    listen<null>(`terminal-exit:${tabId}`, () => {
      term.write("\r\n\x1b[90m[process exited]\x1b[0m\r\n");
    }).then((u) => {
      unlistenExitRef.current = u;
    });

    // ResizeObserver to re-fit whenever container size changes
    const ro = new ResizeObserver(() => fit());
    ro.observe(containerRef.current);

    return () => {
      ro.disconnect();
      // Use refs: local async variables may still be null if unmount was fast
      unlistenDataRef.current?.();
      unlistenExitRef.current?.();
      term.dispose();
      termRef.current = null;
      fitAddonRef.current = null;
    };
  }, [tabId]); // eslint-disable-line react-hooks/exhaustive-deps

  // Re-fit when the panel becomes visible (e.g. switching tabs)
  useEffect(() => {
    if (isVisible) {
      setTimeout(fit, 30);
    }
  }, [isVisible, fit]);

  return (
    <div
      ref={containerRef}
      style={{
        width: "100%",
        height: "100%",
        padding: "4px 0",
        boxSizing: "border-box",
        overflow: "hidden",
      }}
    />
  );
}
