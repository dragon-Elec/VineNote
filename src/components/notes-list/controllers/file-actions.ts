import { rename, remove, writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import { invoke } from "@tauri-apps/api/core";
import type { IFolderItem } from "@/components/navigation-bar/types";
import { buildFrontmatterStr, parseFrontmatter } from "@/utils/markdown-file";

export const createFile = async function (
  selectedFolder: IFolderItem,
  name: string
) {
  const filePath = `${selectedFolder.path}/${name}.md`;
  const content = buildFrontmatterStr({
    title: name,
    tags: [],
    created: new Date().toISOString(),
  });
  await writeTextFile(filePath, content);
  return filePath;
};

export const renameFile = async function (oldPath: string, newPath: string) {
  // Update frontmatter title to reflect the new display name before renaming
  try {
    const raw = await readTextFile(oldPath);
    const { frontmatter, body } = parseFrontmatter(raw);
    const newFileName = newPath.split('/').pop() ?? '';
    const newTitle = newFileName.replace(/(-([^-]+)){1,5}\.(md|json)$/, '');
    await writeTextFile(oldPath, buildFrontmatterStr({ ...frontmatter, title: newTitle }) + body);
  } catch {
    // non-critical: proceed with rename even if frontmatter update fails
  }
  await rename(oldPath, newPath);
};

export const deleteFile = async function (path: string) {
  await remove(path, {
    recursive: true,
  });
};

export const getFiles = async function (folderPath: string): Promise<string> {
  try {
    const result: string = await invoke("get_dir_info", { path: folderPath });
    return result;
  } catch (e) {
    console.error(e);
    return "";
  }
};
