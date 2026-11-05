import {
  readRevitFileDetails,
  RevitFileError,
  type RevitFileErrorCode,
  type RevitFileKind,
  REVIT_KINDS,
} from '@/shared/revit-file/readRevitFileInfo';

/** Files checked in one go, so a big folder cannot exhaust the browser. */
export const MAX_FILES = 500;
/** Files looked at while opening a dropped folder, most of which may not be Revit files. */
const MAX_FOUND = 20_000;
const AT_ONCE = 4;

export interface CheckedFile {
  /** The file's name, with its folder when it came from one. */
  name: string;
  size: number;
  kind: RevitFileKind | null;
  year: number | null;
  build: string | null;
  workshared: boolean | null;
  problem: RevitFileErrorCode | null;
}

/** A file and the name to show for it, with its folder when it came from one. */
export interface PickedFile {
  file: File;
  name: string;
}

export function picked(file: File): PickedFile {
  return { file, name: file.webkitRelativePath || file.name };
}

export function isRevitName(name: string): boolean {
  const lower = name.toLowerCase();
  return REVIT_KINDS.some((kind) => lower.endsWith(`.${kind}`));
}

export async function checkFile({ file, name }: PickedFile): Promise<CheckedFile> {
  const base = { name, size: file.size };
  try {
    return { ...base, ...(await readRevitFileDetails(file)), problem: null };
  } catch (cause) {
    return {
      ...base,
      kind: null,
      year: null,
      build: null,
      workshared: null,
      problem: cause instanceof RevitFileError ? cause.code : 'unreadable',
    };
  }
}

/** Reads every file, a few at a time, keeping their order. Each result reaches `onResult` as it is ready. */
export async function checkFiles(
  files: PickedFile[],
  onResult: (index: number, row: CheckedFile) => void,
): Promise<void> {
  let next = 0;
  const worker = async () => {
    while (next < files.length) {
      const index = next;
      next += 1;
      const file = files[index];
      if (file) {
        onResult(index, await checkFile(file));
      }
    }
  };
  await Promise.all(Array.from({ length: AT_ONCE }, worker));
}

/** One CSV cell. A leading =, +, - or @ would make a spreadsheet run the cell as a formula, so it is neutralized. */
function csvCell(value: string): string {
  const safe = /^[=+\-@\t\r]/.test(value) ? `'${value}` : value;
  return /[",\r\n]/.test(safe) ? `"${safe.replaceAll('"', '""')}"` : safe;
}

/** The table as CSV. The byte order mark makes Excel read non-English text correctly. */
export function csvOf(rows: string[][]): string {
  return `﻿${rows.map((row) => row.map(csvCell).join(',')).join('\r\n')}\r\n`;
}

function filesInEntry(entry: FileSystemEntry, found: { count: number }): Promise<PickedFile[]> {
  if (found.count >= MAX_FOUND) {
    return Promise.resolve([]);
  }
  if (entry.isFile) {
    found.count += 1;
    return new Promise((resolve) => {
      (entry as FileSystemFileEntry).file(
        (file) => {
          resolve([{ file, name: entry.fullPath.replace(/^\//, '') }]);
        },
        () => {
          resolve([]);
        },
      );
    });
  }
  const reader = (entry as FileSystemDirectoryEntry).createReader();
  // A directory hands out its entries in batches, until an empty one.
  const readAll = async (): Promise<PickedFile[]> => {
    const batch = await new Promise<FileSystemEntry[]>((resolve) => {
      reader.readEntries(resolve, () => {
        resolve([]);
      });
    });
    if (batch.length === 0) {
      return [];
    }
    const files = await Promise.all(batch.map((child) => filesInEntry(child, found)));
    return [...files.flat(), ...(await readAll())];
  };
  return readAll();
}

/** The files of a drop, going through any folders in it. */
export async function filesFromDrop(data: DataTransfer): Promise<PickedFile[]> {
  const entries = [...data.items].flatMap((item) => item.webkitGetAsEntry() ?? []);
  if (entries.length === 0) {
    return [...data.files].map(picked);
  }
  const found = { count: 0 };
  return (await Promise.all(entries.map((entry) => filesInEntry(entry, found)))).flat();
}
