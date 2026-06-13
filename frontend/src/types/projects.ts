// File system and project types

export interface FileSystemHandlePermissionDescriptor {
  mode?: 'read' | 'readwrite';
}

export interface FileSystemHandle {
  kind: 'file' | 'directory';
  name: string;
  queryPermission?: (descriptor?: FileSystemHandlePermissionDescriptor) => Promise<PermissionState>;
  requestPermission?: (descriptor?: FileSystemHandlePermissionDescriptor) => Promise<PermissionState>;
}

export interface FileSystemFileHandle extends FileSystemHandle {
  kind: 'file';
  getFile: () => Promise<File>;
  createWritable?: () => Promise<FileSystemWritableFileStream>;
}

export interface FileSystemDirectoryHandle extends FileSystemHandle {
  kind: 'directory';
  entries: () => AsyncIterableIterator<[string, FileSystemFileHandle | FileSystemDirectoryHandle]>;
  getFileHandle?: (name: string, options?: { create?: boolean }) => Promise<FileSystemFileHandle>;
  getDirectoryHandle?: (
    name: string,
    options?: { create?: boolean },
  ) => Promise<FileSystemDirectoryHandle>;
}

export interface FileSystemWritableFileStream extends WritableStream {
  write: (data: string | Blob | BufferSource) => Promise<void>;
  close: () => Promise<void>;
}

export interface ProjectFileSnapshot {
  path: string;
  content: string;
  size: number;
  handle: FileSystemFileHandle;
}

export interface CodeProject {
  id: string;
  name: string;
  fileCount: number;
  totalBytes: number;
  snapshot: string;
  files: ProjectFileSnapshot[];
  writable: boolean;
  updatedAt: string;
  rootHandle: FileSystemDirectoryHandle;
}
