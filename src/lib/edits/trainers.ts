// Trainers in the editor: what to call each one, the story role its tag gives it, and filters by that role.
import type { TrainerPokemonEdit } from '../generated/TrainerPokemonEdit';
import type { EditableGame, EditableTrainer, TrainerLimits } from './types';

/** What a story tag means, e.g. `GYM1-LEADER` → "Gym 1 leader"; `''` for a regular trainer. */
export function tagLabel(tag: string | null): string {
  if (!tag) return '';
  let m: RegExpExecArray | null;
  if ((m = /^GYM(\d+)-LEADER$/.exec(tag))) return 'Gym ' + m[1] + ' leader';
  if ((m = /^GYM(\d+)(-NOTSTRONG)?$/.exec(tag))) return 'Gym ' + m[1] + ' trainer';
  if ((m = /^(RIVAL|FRIEND)(\d+)-(\d)$/.exec(tag))) return (m[1] === 'RIVAL' ? 'Rival' : 'Friend') + ' battle ' + m[2];
  if ((m = /^ELITE(\d+)$/.exec(tag))) return 'Elite Four ' + m[1];
  if (tag === 'CHAMPION') return 'Champion';
  if ((m = /^GIO(\d)$/.exec(tag))) return 'Giovanni ' + m[1];
  if (tag === 'UBER') return 'Postgame';
  if (tag === 'STRONG') return 'Strong trainer';
  if (tag.startsWith('THEMED:')) {
    const raw = tag.slice(7).replace(/-(STRONG|LEADER|NOTSTRONG)/g, '').replace(/\d+$/, '');
    const named: Record<string, string> = { SPROUTTOWER: 'Sprout Tower', 'JESSIE&JAMES': 'Jessie & James', SHADOW: 'Shadow Triad' };
    return 'Story: ' + (named[raw] ?? raw.charAt(0) + raw.slice(1).toLowerCase());
  }
  return '';
}

/** The trainer filters: an id, a label, and which tags it takes. */
export const TRAINER_FILTERS = [
  ['all', 'All'],
  ['gym', 'Gyms'],
  ['elite', 'Elite Four & Champion'],
  ['rival', 'Rivals & friends'],
  ['story', 'Story battles'],
  ['post', 'Postgame']
] as const;
export type TrainerFilter = (typeof TRAINER_FILTERS)[number][0];

/** Whether a trainer with `tag` belongs under `filter`. */
export function inFilter(filter: TrainerFilter, tag: string | null): boolean {
  const t = tag ?? '';
  switch (filter) {
    case 'all':
      return true;
    case 'gym':
      return /^GYM/.test(t);
    case 'elite':
      return /^(ELITE|CHAMPION)/.test(t);
    case 'rival':
      return /^(RIVAL|FRIEND)/.test(t);
    case 'story':
      return /^(THEMED|GIO)/.test(t);
    case 'post':
      return t === 'UBER';
  }
}

/** The filters some trainer of the game falls under, "All" first. */
export function trainerFilters(trainers: readonly EditableTrainer[]): (typeof TRAINER_FILTERS)[number][] {
  return TRAINER_FILTERS.filter(([id]) => id === 'all' || trainers.some((t) => inFilter(id, t.tag)));
}

/**
 * What to call each trainer, in the game's order: class and name ("Leader Roxanne"), or, for a class whose trainers
 * have no names, the class with a running number when it has several ("Rival #2").
 */
export function trainerLabels(trainers: readonly EditableTrainer[]): string[] {
  const cls = (t: EditableTrainer) => (/^Rival\d$/.test(t.className) ? 'Rival' : t.className || 'Trainer');
  const size = new Map<string, number>();
  for (const t of trainers) size.set(cls(t), (size.get(cls(t)) ?? 0) + 1);
  const seen = new Map<string, number>();
  return trainers.map((t) => {
    const c = cls(t);
    seen.set(c, (seen.get(c) ?? 0) + 1);
    if (/[A-Za-z]/.test(t.name)) return c + ' ' + t.name;
    return (size.get(c) ?? 0) > 1 ? c + ' #' + seen.get(c) : c;
  });
}

/** The limits of trainer `index`, or the ones the edit model allows any team when the rules have none for it. */
export function limitsOf(view: EditableGame, index: number): TrainerLimits {
  const r = view.rules.trainers;
  return r.trainers.find((t) => t.index === index) ?? { index, minPokemon: 1, maxPokemon: 6, sharesBattle: false, levels: true, heldItems: r.heldItems, customMoves: r.customMoves, starterSlot: null };
}

/** "3 of 6 Pokémon". */
export function teamText(n: number, max: number): string {
  return n + ' of ' + max + ' Pokémon';
}

/** The Pokémon "Add Pokémon" puts at the end of a team: the last one again, without an item or moves of its own. */
export function addedPokemon(team: readonly TrainerPokemonEdit[]): TrainerPokemonEdit {
  const last = team[team.length - 1];
  return { species: last?.species ?? 25, level: last?.level ?? 5, heldItem: null, moves: null };
}

/** A team's moves with move `k` of Pokémon `slot` set to `move`; all four empty means its level-up moves again. */
export function withMove(moves: TrainerPokemonEdit['moves'], k: number, move: number): TrainerPokemonEdit['moves'] {
  const out: [number, number, number, number] = moves ? [...moves] : [0, 0, 0, 0];
  out[k] = move;
  return out.every((m) => m === 0) ? null : out;
}

/** The note under a team: what the game stores per team. */
export function teamNote(limits: TrainerLimits, generation: number): string {
  if (generation === 1) return 'Trainers in this game carry no held items and always use their level-up moves. A class whose teams grow moves to free space in its bank.';
  const parts: string[] = [];
  if (limits.heldItems || limits.customMoves) parts.push('Held items and custom moves are stored per team: once one Pokémon has an item or a custom move, the others keep a slot too.');
  if (limits.customMoves) parts.push('Default means the level-up moves for that level.');
  if (!limits.heldItems && !limits.customMoves) parts.push('This trainer’s Pokémon can’t hold items or have moves of their own.');
  if (!limits.levels) parts.push('The game sets the levels of this trainer’s Pokémon.');
  return parts.join(' ');
}
