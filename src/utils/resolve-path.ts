import { convertFileSrc } from '@tauri-apps/api/core';

export const resolvePath = (docPath: string, relativePath: string): string => {
  const normalizedDoc = docPath.replace(/\\/g, '/');
  const normalizedRel = relativePath.replace(/\\/g, '/');
  
  // If it's already absolute (starts with / or has drive letter on Windows)
  if (normalizedRel.startsWith('/') || /^[a-zA-Z]:\//.test(normalizedRel)) {
    return normalizedRel;
  }

  const dir = normalizedDoc.substring(0, normalizedDoc.lastIndexOf('/'));
  const parts = dir.split('/');
  const relParts = normalizedRel.split('/');
  
  for (const part of relParts) {
    if (part === '.' || part === '') continue;
    if (part === '..') {
      if (parts.length > 1) {
        parts.pop();
      }
    } else {
      parts.push(part);
    }
  }
  
  return parts.join('/');
};

export const resolveRelativeAsset = (docPath: string, assetPath: string): string => {
  if (!assetPath) return '';
  if (
    assetPath.startsWith('http://') ||
    assetPath.startsWith('https://') ||
    assetPath.startsWith('tauri://') ||
    assetPath.startsWith('data:')
  ) {
    return assetPath;
  }
  
  try {
    const absPath = resolvePath(docPath, assetPath);
    return convertFileSrc(absPath);
  } catch (e) {
    console.error('Failed to resolve relative asset path:', e);
    return assetPath;
  }
};
