import { writeTextFile, readTextFile } from '@tauri-apps/plugin-fs';
import { serializeMd, deserializeMd } from '@udecode/plate-markdown';
import type { SlateEditor, Descendant } from '@udecode/plate';
import { parseFrontmatter, buildFrontmatterStr, type Frontmatter } from '@/utils/markdown-file';

export interface FileContent {
  nodes: Descendant[];
  frontmatter: Frontmatter;
}

export const writeToFile = async function (
  editor: SlateEditor,
  content: Descendant[],
  filePath: string,
  metaOverride?: Partial<Frontmatter>
) {
  // Preserve existing frontmatter fields
  let existingFrontmatter: Frontmatter = {};
  try {
    const raw = await readTextFile(filePath);
    existingFrontmatter = parseFrontmatter(raw).frontmatter;
  } catch {
    // new or unreadable file — start fresh
  }

  const markdown = serializeMd(editor, { value: content });
  const frontmatter: Frontmatter = {
    ...existingFrontmatter,
    ...metaOverride,
    modified: new Date().toISOString(),
  };

  await writeTextFile(filePath, buildFrontmatterStr(frontmatter) + markdown);
};

export const readFileAsNodes = async function (
  editor: SlateEditor,
  filePath: string
): Promise<FileContent> {
  let raw = '';
  try {
    raw = await readTextFile(filePath);
  } catch (e) {
    console.log('read error', e);
  }

  if (!raw.trim()) {
    return { nodes: [], frontmatter: {} };
  }

  // Legacy JSON format — transparently handle existing .json notes
  const trimmed = raw.trimStart();
  if (trimmed.startsWith('[') || trimmed.startsWith('{')) {
    try {
      return { nodes: JSON.parse(raw), frontmatter: {} };
    } catch {
      return { nodes: [], frontmatter: {} };
    }
  }

  const { frontmatter, body } = parseFrontmatter(raw);
  const nodes = body.trim() ? deserializeMd(editor, body) : [];
  return { nodes: nodes as Descendant[], frontmatter };
};

/** Read only frontmatter without deserializing body (no editor needed). */
export const readFileFrontmatter = async function (
  filePath: string
): Promise<Frontmatter> {
  try {
    const raw = await readTextFile(filePath);
    return parseFrontmatter(raw).frontmatter;
  } catch {
    return {};
  }
};