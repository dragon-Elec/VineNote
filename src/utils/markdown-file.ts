export interface Frontmatter {
  id?: string;
  title?: string;
  tags?: string[];
  created?: string;
  modified?: string;
  [key: string]: any;
}

export interface ParsedMarkdownFile {
  frontmatter: Frontmatter;
  body: string;
}

/**
 * Parse a markdown string with optional YAML frontmatter.
 * Frontmatter is delimited by --- lines at the start of the file.
 */
export function parseFrontmatter(raw: string): ParsedMarkdownFile {
  if (!raw.startsWith('---')) {
    return { frontmatter: {}, body: raw };
  }

  // Find the closing ---
  const afterFirst = raw.slice(3);
  const endIdx = afterFirst.indexOf('\n---');
  if (endIdx < 0) {
    return { frontmatter: {}, body: raw };
  }

  const yamlStr = afterFirst.slice(0, endIdx).replace(/^\n/, '');
  const body = afterFirst.slice(endIdx + 4).replace(/^\n/, '');

  const frontmatter: Frontmatter = {};

  yamlStr.split('\n').forEach(line => {
    const colonIdx = line.indexOf(':');
    if (colonIdx < 0) return;
    const key = line.slice(0, colonIdx).trim();
    const value = line.slice(colonIdx + 1).trim();
    if (!key) return;

    if (value.startsWith('[') && value.endsWith(']')) {
      const inner = value.slice(1, -1).trim();
      frontmatter[key] = inner
        ? inner.split(',').map(s => s.trim()).filter(Boolean)
        : [];
    } else {
      frontmatter[key] = value;
    }
  });

  return { frontmatter, body };
}

/**
 * Serialize a frontmatter object to a YAML block with --- delimiters.
 * Returns an empty string if meta is empty.
 */
export function buildFrontmatterStr(meta: Frontmatter): string {
  const entries = Object.entries(meta).filter(
    ([, v]) => v !== undefined && v !== null
  );
  if (entries.length === 0) return '';

  const lines: string[] = ['---'];

  for (const [key, value] of entries) {
    if (Array.isArray(value)) {
      lines.push(`${key}: [${value.join(', ')}]`);
    } else {
      lines.push(`${key}: ${value}`);
    }
  }

  lines.push('---');
  lines.push('');

  return lines.join('\n');
}
