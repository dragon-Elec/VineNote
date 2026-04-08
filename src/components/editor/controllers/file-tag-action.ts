// tag action for the file — tags are stored in each note's frontmatter
import type { ITagItem } from "@/components/navigation-bar/types";
import type { IArticleItem } from "@/components/notes-list/types";
import { readTextFile, writeTextFile } from "@tauri-apps/plugin-fs";
import { parseFrontmatter, buildFrontmatterStr } from "@/utils/markdown-file";

async function updateFileTags(
  filePath: string,
  updater: (tags: string[]) => string[]
): Promise<void> {
  const raw = await readTextFile(filePath);
  const { frontmatter, body } = parseFrontmatter(raw);
  const existingTags: string[] = Array.isArray(frontmatter.tags)
    ? frontmatter.tags
    : [];
  const newTags = updater(existingTags);
  const newFrontmatter = { ...frontmatter, tags: newTags };
  await writeTextFile(filePath, buildFrontmatterStr(newFrontmatter) + body);
}

export const addTagForFile = async function (
  dataSource: ITagItem[],
  _tagFilePath: string,
  tagItem: ITagItem,
  targetFile: IArticleItem
): Promise<ITagItem[]> {
  await updateFileTags(targetFile.path, (tags) => {
    if (tags.includes(tagItem.name)) return tags;
    return [...tags, tagItem.name];
  });

  // Keep in-memory store consistent
  const existingIndex = dataSource.findIndex((t) => t.name === tagItem.name);
  if (existingIndex < 0) {
    return [...dataSource, { ...tagItem, files: [targetFile] }];
  }
  const updated = dataSource.map((t, i) => {
    if (i !== existingIndex) return t;
    if (t.files.some((f) => f.path === targetFile.path)) return t;
    return { ...t, files: [...t.files, targetFile] };
  });
  return updated;
};

export const deleteTagForFile = async function (
  dataSource: ITagItem[],
  _tagFilePath: string,
  tagItem: ITagItem,
  targetFile: IArticleItem
): Promise<ITagItem[]> {
  await updateFileTags(targetFile.path, (tags) =>
    tags.filter((t) => t !== tagItem.name)
  );

  const updated = dataSource.map((t) => {
    if (t.name !== tagItem.name) return t;
    return { ...t, files: t.files.filter((f) => f.path !== targetFile.path) };
  });
  return updated;
};
