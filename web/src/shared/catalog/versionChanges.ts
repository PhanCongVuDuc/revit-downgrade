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

/** Autodesk's help for a Revit release, where its "What's New" pages list everything that release added. */
export function revitHelpUrl(year: number): string {
  return `https://help.autodesk.com/view/RVT/${String(year)}/ENU/`;
}
