import { invoke } from "@tauri-apps/api/core";
import { uid } from "uid";
import type { ITagItem } from "../types";
import type { IArticleItem } from '@/components/notes-list/types';
import getNavPath from "@/utils/get-nav-path";

// Raw result from the Rust command
interface RawTagEntry {
  tag_name: string;
  files: string[];
}

/**
 * Scan the notes directory for all .md files and collect tags from frontmatter.
 * Replaces the old list.json approach.
 */
export const getTagList = async function (): Promise<{
  filePath: string;
  dataSource: ITagItem[];
}> {
  const notesDirPath = await getNavPath("notes");
  let raw: RawTagEntry[] = [];
  try {
    raw = await invoke<RawTagEntry[]>("get_tags_from_dir", { path: notesDirPath });
  } catch (e) {
    console.error("get_tags_from_dir failed", e);
  }

  // Build ITagItem[] with file stubs — path only (id/name come from filename)
  const dataSource: ITagItem[] = raw.map((entry) => {
    return {
      id: uid(),
      name: entry.tag_name,
      files: entry.files.map((filePath) => {
        const parts = filePath.split("/");
        const rawName = parts[parts.length - 1];
        // Strip the uid suffix (same pattern used in notes-list/index.tsx)
        const name = rawName.replace(/(-([^-]+)){1,5}\.(md|json)$/, "");
        return {
          id: uid(),
          name,
          path: filePath,
          metadata: { is_file: true, is_dir: false, created: "" },
        } as IArticleItem;
      }),
    };
  });

  return { filePath: notesDirPath, dataSource };
};

/** Returns the notes directory path (kept for API compatibility). */
export const getTagFilePath = async function (): Promise<string> {
  return getNavPath("notes");
};

/**
 * Create a brand-new tag (in-memory only — tags persist via note frontmatter).
 * Callers should call addTagForFile after this to attach it to a note.
 */
export const createTag = async function (
  _filePath: string,
  tagName: string
): Promise<ITagItem> {
  return { id: uid(), name: tagName, files: [] };
};

/**
 * Delete a tag by name: remove the tag from every .md file that holds it.
 */
export const deleteTag = async function (
  _filePath: string,
  tagName: string
): Promise<void> {
  const notesDirPath = await getNavPath("notes");
  await invoke("remove_tag_from_all_files", { path: notesDirPath, tagName });
};

/**
 * Rename a tag: update the tag string in every .md file that holds it.
 */
export const renameTag = async function (
  _filePath: string,
  oldName: string,
  newName: string
): Promise<void> {
  const notesDirPath = await getNavPath("notes");
  await invoke("rename_tag_in_all_files", { path: notesDirPath, oldName, newName });
};

/**
 * When a note file is deleted, there is nothing to sync — the file is gone
 * and the tag index is rebuilt from disk on next load.
 */
export const syncDeletedFile2Tag = async function (
  _deletedFile: IArticleItem
): Promise<void> {
  // no-op: tags live in file frontmatter; deleting the file removes the tags too
};

/**
 * When a note file is renamed, the new path already has the correct frontmatter
 * (written by the rename operation). Nothing extra to sync.
 */
export const syncRenamedFile2Tag = async function (
  _originFile: IArticleItem,
  _renamedFile: IArticleItem
): Promise<void> {
  // no-op: file-tag-action writeToFile preserves frontmatter on rename
};


