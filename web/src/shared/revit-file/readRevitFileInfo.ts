import { OleError, readStream } from './ole';

export const REVIT_KINDS = ['rvt', 'rte', 'rfa', 'rft'] as const;
export type RevitFileKind = (typeof REVIT_KINDS)[number];

export interface RevitFileInfo {
  kind: RevitFileKind;
  year: number;
}

/** What the version checker shows of a file. */
export interface RevitFileDetails extends RevitFileInfo {
  /** The Revit build that saved it, e.g. 20250227_1515(x64). */
  build: string | null;
  /** Whether worksharing is on in the file. Null when the file does not say. */
  workshared: boolean | null;
}

export type RevitFileErrorCode = 'bad_extension' | 'too_large' | 'not_ole' | 'not_revit' | 'unreadable';

export class RevitFileError extends Error {
  readonly code: RevitFileErrorCode;

  constructor(code: RevitFileErrorCode, message: string) {
    super(message);
    this.name = 'RevitFileError';
    this.code = code;
  }
}

/** 1 GiB, the largest upload the server accepts. */
export const MAX_FILE_BYTES = 1_073_741_824;

const YEAR_PATTERN = /Format: (\d{4})/;
const BUILD_PATTERN = /Build: (\d{8}_\d{4}(?:\(x\d+\))?)/;
const WORKSHARING_PATTERN = /Worksharing: ([^\r\n]*)/;

function kindOf(name: string): RevitFileKind | null {
  const dot = name.lastIndexOf('.');
  if (dot <= 0) {
    return null;
  }
  const extension = name.slice(dot + 1).toLowerCase();
  return REVIT_KINDS.find((kind) => kind === extension) ?? null;
}

/** The text is UTF-16LE, but the binary prefix before it may leave it at an odd offset, so both alignments are tried. */
function textWithYear(stream: Uint8Array): { text: string; year: number } | null {
  const decoder = new TextDecoder('utf-16le');
  for (const shift of [0, 1]) {
    const text = decoder.decode(stream.subarray(shift));
    const match = YEAR_PATTERN.exec(text);
    if (match?.[1]) {
      return { text, year: Number(match[1]) };
    }
  }
  return null;
}

async function readText(file: Pick<Blob, 'size' | 'slice'>): Promise<{ text: string; year: number }> {
  let stream: Uint8Array;
  try {
    stream = await readStream(file, 'BasicFileInfo');
  } catch (error) {
    if (error instanceof OleError) {
      switch (error.code) {
        case 'not_ole':
          throw new RevitFileError('not_ole', error.message);
        case 'not_found':
          throw new RevitFileError('not_revit', 'the file has no BasicFileInfo stream');
        case 'unreadable':
          throw new RevitFileError('unreadable', error.message);
      }
    }
    throw new RevitFileError('unreadable', error instanceof Error ? error.message : String(error));
  }
  const found = textWithYear(stream);
  if (found === null) {
    throw new RevitFileError('not_revit', 'BasicFileInfo names no format year');
  }
  return found;
}

/**
 * The kind (from the extension) and the format year (from the `BasicFileInfo` stream) of a Revit file, read in the browser
 * through a few small slices. Throws `RevitFileError`. Whether the year can be converted is the caller's question.
 */
export async function readRevitFileInfo(file: Pick<Blob, 'size' | 'slice'> & { name: string }): Promise<RevitFileInfo> {
  const kind = kindOf(file.name);
  if (!kind) {
    throw new RevitFileError('bad_extension', 'only .rvt, .rte, .rfa and .rft files are accepted');
  }
  if (file.size > MAX_FILE_BYTES) {
    throw new RevitFileError('too_large', 'the file is larger than 1 GiB');
  }
  const { year } = await readText(file);
  return { kind, year };
}

/** As `readRevitFileInfo`, with the build and worksharing as well, and for a file of any size: nothing here is uploaded. */
export async function readRevitFileDetails(
  file: Pick<Blob, 'size' | 'slice'> & { name: string },
): Promise<RevitFileDetails> {
  const kind = kindOf(file.name);
  if (!kind) {
    throw new RevitFileError('bad_extension', 'only .rvt, .rte, .rfa and .rft files are accepted');
  }
  const { text, year } = await readText(file);
  const worksharing = WORKSHARING_PATTERN.exec(text)?.[1]?.trim();
  return {
    kind,
    year,
    build: BUILD_PATTERN.exec(text)?.[1] ?? null,
    workshared: worksharing ? worksharing !== 'Not enabled' : null,
  };
}
