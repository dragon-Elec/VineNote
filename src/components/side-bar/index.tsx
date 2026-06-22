import NavigationBar from "../navigation-bar";
import NotesList from "../notes-list";
import useFocusMode from "../editor/controllers/focus-mode";
import { useAppMode } from "../editor/controllers/app-mode";
import { StandaloneSideBar } from "./standalone-sidebar";
import { cn } from "@/lib/utils";
import { motion } from "motion/react";
import styles from "./index.module.css";

const SideBar = function () {
  const isEditorInFocusMode = useFocusMode((state) => state.isFocusMode);
  const appMode = useAppMode((state) => state.appMode);

  return (
    <motion.div
      className={cn(styles.side_bar)}
      animate={{
        display: isEditorInFocusMode ? "none" : "flex",
        transition: {
          type: 'tween',
          duration: 0.2
        }
      }}
    >
      {appMode === "StandaloneEdit" ? (
        <StandaloneSideBar />
      ) : (
        <>
          <NavigationBar />
          <NotesList />
        </>
      )}
    </motion.div>
  );
};

export default SideBar;
