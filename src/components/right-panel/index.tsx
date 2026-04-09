import { useRef, useCallback, useEffect } from "react";
import { motion, AnimatePresence, useMotionValue, animate } from "motion/react";
import { MessageSquare, SquareTerminal, X } from "lucide-react";
import { useRightPanel, type RightPanelTab } from "./use-right-panel";
import { ChatPanel } from "./chat";
import { TerminalPanel } from "./terminal";
import styles from "./index.module.css";

interface TabDef {
  id: RightPanelTab;
  label: string;
  Icon: React.ElementType;
}

const TABS: TabDef[] = [
  { id: "chat", label: "Chat", Icon: MessageSquare },
  { id: "terminal", label: "Terminal", Icon: SquareTerminal },
];

export function RightPanel() {
  const { isOpen, activeTab, width, close, setTab, setWidth, snapWidth, clampWidth } =
    useRightPanel();

  // motionWidth drives the actual CSS width so we can bypass Framer Motion
  // animation during live drag (direct set) and only animate on open/close/snap.
  const motionWidth = useMotionValue(width);

  const isDragging = useRef(false);
  const dragStartX = useRef(0);
  const dragStartWidth = useRef(0);

  // Sync motionWidth when Zustand width changes (snap or initial open)
  useEffect(() => {
    if (!isDragging.current) {
      // Animate for snap / open transition
      animate(motionWidth, width, { duration: 0.15, ease: [0.16, 1, 0.3, 1] });
    }
  }, [width, motionWidth]);

  // ── Resize handle drag ───────────────────────────────────────────────────
  const onMouseDownHandle = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      isDragging.current = true;
      dragStartX.current = e.clientX;
      dragStartWidth.current = width;
      document.body.style.cursor = "ew-resize";
      document.body.style.userSelect = "none";
    },
    [width]
  );

  useEffect(() => {
    const onMouseMove = (e: MouseEvent) => {
      if (!isDragging.current) return;
      const delta = dragStartX.current - e.clientX;
      const next = clampWidth(dragStartWidth.current + delta);
      // Set motionWidth directly (no animation) for zero-lag drag
      motionWidth.set(next);
      // Keep Zustand store in sync (for xterm fit callbacks)
      setWidth(next);
    };

    const onMouseUp = (e: MouseEvent) => {
      if (!isDragging.current) return;
      isDragging.current = false;
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      const delta = dragStartX.current - e.clientX;
      // snapWidth commits to the nearest snap point and triggers the spring above
      snapWidth(dragStartWidth.current + delta);
    };

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [setWidth, snapWidth, clampWidth, motionWidth]);

  return (
    <AnimatePresence>
      {isOpen && (
        <motion.div
          className={styles.panel}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ opacity: { duration: 0.15 } }}
          style={{ minWidth: 0, width: motionWidth }}
        >
          {/* Resize handle */}
          <div
            className={styles.resize_handle}
            onMouseDown={onMouseDownHandle}
          />

          {/* Panel content */}
          <div className={styles.panel_inner}>
            {/* Tab Bar */}
            <div className={styles.tab_bar}>
              <div className={styles.tab_list}>
                {TABS.map(({ id, label, Icon }) => {
                  const isActive = activeTab === id;
                  return (
                    <button
                      key={id}
                      onClick={() => setTab(id)}
                      className={styles.tab_btn}
                      data-active={isActive}
                    >
                      <Icon size={13} strokeWidth={isActive ? 2 : 1.5} />
                      <span>{label}</span>
                    </button>
                  );
                })}
              </div>

              <button
                className={styles.close_btn}
                onClick={close}
                title="Close panel"
              >
                <X size={13} />
              </button>
            </div>

            {/* Content area — render both, show only the active one */}
            <div className={styles.content}>
              <div
                className={styles.pane}
                style={{ display: activeTab === "chat" ? "flex" : "none" }}
              >
                <ChatPanel />
              </div>
              <div
                className={styles.pane}
                style={{ display: activeTab === "terminal" ? "flex" : "none" }}
              >
                <TerminalPanel />
              </div>
            </div>
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
