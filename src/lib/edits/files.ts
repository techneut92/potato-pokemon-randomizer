// Edits files: the loaded game's manual edits saved as JSON, and loaded again (ADR 0008).
import { BackendError } from '../backend';
import { editsText } from './sections';

/** The extension of edits files; keep in step with `EDITS_EXTENSION` in `ppr-service`. They are JSON underneath. */
export const EDITS_EXTENSION = 'ppredits.json';

/** The name an edits file gets by default: the game's, as the app shows it, e.g. `Pokémon Emerald (U).ppredits.json`. */
export function editsFileName(gameName: string): string {
  const safe = gameName.replace(/[\\/:*?"<>|]/g, '').trim() || 'Manual edits';
  return safe + '.' + EDITS_EXTENSION;
}

/** `/games/edits.ppredits.json` → `edits.ppredits.json`. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** What to tell the user when an edits file can't be loaded. */
export function importError(e: unknown): string {
  if (!(e instanceof BackendError)) return 'The edits file couldn’t be loaded.';
  switch (e.code) {
    case 'edits_wrong_game':
      // The backend names both games: "This file is for Pokémon Ruby (U), but Pokémon Emerald (U) is loaded."
      return e.message;
    case 'edits_too_new':
      return 'These edits were saved by a newer randomizer version. Try downloading the latest version.';
    case 'edits_unsupported_version':
      return 'These edits are in an older format this version can’t read.';
    case 'edits_invalid':
      return 'This file isn’t a manual edits file, or it is damaged.';
    case 'open_failed':
      return 'The edits file couldn’t be opened.';
    default:
      return e.message;
  }
}

/** The question before a loaded file replaces the edits the game has. */
export function replaceQuestion(gameName: string, current: number, file: string, loaded: number): string {
  const then = loaded ? `replaces them with the ${editsText(loaded)} it holds` : 'removes them, as it holds none';
  return `${gameName} has ${editsText(current)}. Loading ${file} ${then}.`;
}
