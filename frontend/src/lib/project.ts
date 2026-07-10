// Project file scanning and snapshot building utilities
import type { ProjectFileSnapshot, FileSystemDirectoryHandle } from '../types/index.js';
import {
  MAX_PROJECT_FILES,
  MAX_PROJECT_FILE_BYTES,
  MAX_PROJECT_CONTEXT_CHARS,
  IGNORED_PROJECT_DIRECTORIES,
  IGNORED_PROJECT_FILES,
  CODE_PROJECT_EXTENSIONS,
} from '../constants/index.js';

export function projectFileExtension(path: string) {
  const dotIndex = path.lastIndexOf('.');
  return dotIndex >= 0 ? path.slice(dotIndex).toLowerCase() : '';
}

export function shouldReadProjectFile(path: string, size: number) {
  const fileName = path.split('/').pop() ?? path;
  return size <= MAX_PROJECT_FILE_BYTES && !IGNORED_PROJECT_FILES.has(fileName) && CODE_PROJECT_EXTENSIONS.has(projectFileExtension(path));
}

export async function scanProjectDirectory(
  directoryHandle: FileSystemDirectoryHandle,
  prefix = '',
  files: ProjectFileSnapshot[] = [],
) {
  for await (const [name, handle] of directoryHandle.entries()) {
    if (files.length >= MAX_PROJECT_FILES) break;
    const path = prefix ? `${prefix}/${name}` : name;
    if (handle.kind === 'directory') {
      if (!IGNORED_PROJECT_DIRECTORIES.has(name)) await scanProjectDirectory(handle, path, files);
      continue;
    }
    const file = await handle.getFile();
    if (!shouldReadProjectFile(path, file.size)) continue;
    try { files.push({ path, content: await file.text(), size: file.size, handle }); }
    catch { /* skip files browser cannot decode */ }
  }
  return files;
}

export function buildProjectSnapshot(projectName: string, files: ProjectFileSnapshot[]) {
  const sortedFiles = [...files].sort((a, b) => a.path.localeCompare(b.path));
  const toc = sortedFiles.map((f) => `- ${f.path} (${f.size} bytes)`).join('\n');
  let snapshot = `PROJECT: ${projectName}\nFILES SCANNED: ${files.length}\n\nFILE TREE:\n${toc}\n`;
  for (const file of sortedFiles) {
    const next = `\n\n--- FILE: ${file.path} ---\n${file.content}`;
    if (snapshot.length + next.length > MAX_PROJECT_CONTEXT_CHARS) {
      snapshot += '\n\n[AEGIS truncated the project snapshot to fit the model context budget.]';
      break;
    }
    snapshot += next;
  }
  return snapshot;
}

export function findProjectFile(project: { files: ProjectFileSnapshot[] }, path: string) {
  const normalizedPath = path.replace(/^[/\\]+/, '').replace(/\\/g, '/');
  const exact = project.files.find((file) => file.path === normalizedPath);
  if (exact) return exact;
  // Try matching by filename (last segment) when exact path doesn't match
  const filename = normalizedPath.split('/').pop() || normalizedPath;
  return project.files.find((file) => file.path.endsWith('/' + filename) || file.path === filename) || null;
}
