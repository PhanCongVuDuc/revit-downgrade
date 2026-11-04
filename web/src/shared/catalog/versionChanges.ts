/**
 * Versions whose step down has its own notes: `n` means the step from Revit n to Revit n − 1. The text sits in the
 * convert locale files under `convert.changes.<n>`: `removed` lists the kinds of elements Revit n − 1 has no place for,
 * `other` (when present) what else changes on the way down.
 */
export const VERSION_STEPS: { year: number; other: boolean }[] = [
  { year: 2027, other: true },
  { year: 2026, other: true },
  { year: 2025, other: true },
  { year: 2024, other: false },
  { year: 2023, other: false },
];

/** The "What's New in Revit <year>" page of Autodesk's help, which lists everything that release added. */
export function revitHelpUrl(year: number): string {
  // The page has the same id in every release's help.
  return `https://help.autodesk.com/view/RVT/${String(year)}/ENU/?guid=GUID-C81929D7-02CB-4BF7-A637-9B98EC9EB38B`;
}
