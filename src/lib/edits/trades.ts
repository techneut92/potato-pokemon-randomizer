// In-game trades in the editor: which fields a game's trades have, and how their IVs are labelled.
import type { TradeRules } from './types';

/** The label of each IV: four DVs in Gen 2, six IVs later. */
export function ivLabels(count: number): string[] {
  return count === 4 ? ['Atk', 'Def', 'Spe', 'Spc'] : ['HP', 'Atk', 'Def', 'Spe', 'SpA', 'SpD'].slice(0, count);
}

/** The legend over the IV fields. */
export function ivLegend(r: TradeRules): string {
  return (r.maxIv === 15 ? 'DVs' : 'IVs') + ' (0–' + r.maxIv + (r.randomIv !== null ? ', empty means random)' : ')');
}

/** An IV as typed: empty is the game's random value where it has one, otherwise the number within 0 and the maximum. */
export function readIv(text: string, r: TradeRules): number {
  const raw = text.trim();
  if (raw === '' && r.randomIv !== null) return r.randomIv;
  const n = Math.round(Number(raw));
  return Number.isFinite(n) ? Math.max(0, Math.min(r.maxIv, n)) : 0;
}

/** An IV as shown: empty for the random value. */
export function shownIv(iv: number, r: TradeRules): string {
  return r.randomIv !== null && iv === r.randomIv ? '' : String(iv);
}

/** The note under the trades. */
export function tradeNote(r: TradeRules): string {
  if (!r.texts) {
    const what = r.originalTrainer ? 'Nicknames and original trainer names' : 'Nicknames';
    return what + ' stay as they are in this game.' + (r.originalTrainer ? '' : ' Trades in this game set no original trainer, IVs or held item.');
  }
  const nick = 'Nicknames take up to ' + r.maxNicknameLength + ' characters';
  if (!r.originalTrainer) return nick + '. Trades in this game set no original trainer, IVs or held item.';
  return nick + ' and original trainer names up to ' + r.maxOtNameLength + '.';
}
