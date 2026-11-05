import { describe, expect, it } from 'vitest';

import { checkFile, checkFiles, csvOf, isRevitName, picked } from './checkFiles';

describe('isRevitName', () => {
  it('knows the four Revit extensions in any case', () => {
    expect(['a.rvt', 'B.RFA', 'c.rte', 'd.Rft'].every(isRevitName)).toBe(true);
    expect(isRevitName('notes.txt')).toBe(false);
    expect(isRevitName('rvt')).toBe(false);
  });
});

describe('checkFile', () => {
  it('reports a file that is not a Revit file instead of failing', async () => {
    const row = await checkFile(picked(new File(['plain text'], 'House.rvt')));
    expect(row).toMatchObject({ name: 'House.rvt', size: 10, year: null, problem: 'not_ole' });
  });
});

describe('checkFiles', () => {
  it('gives every file its own place in the order', async () => {
    const files = ['a', 'b', 'c', 'd', 'e', 'f'].map((name) => picked(new File(['x'], `${name}.rvt`)));
    const rows: string[] = [];
    await checkFiles(files, (index, row) => {
      rows[index] = row.name;
    });
    expect(rows).toEqual(['a.rvt', 'b.rvt', 'c.rvt', 'd.rvt', 'e.rvt', 'f.rvt']);
  });
});

describe('csvOf', () => {
  it('quotes what needs it and starts with a byte order mark', () => {
    expect(csvOf([['a,b', 'say "hi"', 'plain']])).toBe('﻿"a,b","say ""hi""",plain\r\n');
  });

  it('keeps a file name from running as a formula in a spreadsheet', () => {
    expect(csvOf([['=HYPERLINK("x").rvt']])).toBe('﻿"\'=HYPERLINK(""x"").rvt"\r\n');
    expect(csvOf([['-1.rvt', '@a.rvt', '+b.rvt']])).toBe("﻿'-1.rvt,'@a.rvt,'+b.rvt\r\n");
  });
});
