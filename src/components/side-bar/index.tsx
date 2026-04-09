import NavigationBar from "../navigation-bar";
import NotesList from "../notes-list";
import SourcesPanel from "../pipeline/sources";
import InboxPanel from "../pipeline/inbox";
import CardsPanel from "../pipeline/cards";
import useFocusMode from "../editor/controllers/focus-mode";
import { useSelectedNav } from "../navigation-bar/controllers/selected-nav";
import { cn } from "@/lib/utils";
import { motion } from "motion/react";
import styles from "./index.module.css";

const PIPELINE_MODES = ["sources", "inbox", "cards"];

const SideBar = function () {
  const isEditorInFocusMode = useFocusMode((state) => state.isFocusMode);
  const selectedNav = useSelectedNav((s) => s.selectedNav);
  const isPipelineMode = PIPELINE_MODES.includes(selectedNav);

  return (
    <motion.div
      className={cn(styles.side_bar)}
      animate={{
        display: isEditorInFocusMode ? "none" : "flex",
        transition: { type: "tween", duration: 0.2 },
      }}
    >
      <NavigationBar />
      {!isPipelineMode && <NotesList />}
      {selectedNav === "sources" && (
        <div className={styles.pipeline_panel}>
          <SourcesPanel />
        </div>
      )}
      {selectedNav === "inbox" && (
        <div className={styles.pipeline_panel}>
          <InboxPanel />
        </div>
      )}
      {selectedNav === "cards" && (
        <div className={styles.pipeline_panel}>
          <CardsPanel />
        </div>
      )}
    </motion.div>
  );
};

export default SideBar;

