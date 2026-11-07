import type { Ctx, GroupDef, OptDef, Overrides, SectionDef } from './types';
import type { RomInfo } from '../data/roms';
import { TWEAKS } from '../data/roms';
import { DEX } from '../data/dex';
import { tip } from '../tips';

const TYPES = ['Random', 'Normal', 'Fighting', 'Flying', 'Poison', 'Ground', 'Rock', 'Bug', 'Ghost', 'Steel', 'Fire', 'Water', 'Grass', 'Electric', 'Psychic', 'Ice', 'Dragon', 'Dark'];

export const optKey = (g: GroupDef, o: OptDef, oi: number) => g.key + '.' + (o.key ?? 'o' + oi);

export const NOT_PORTED = 'Not available yet: this part of the randomizer hasn’t been built for real games.';

/** Why a group can't be used with this ROM: the game lacks it, or the new engine doesn't do it yet. */
function groupNa(g: GroupDef, rom: RomInfo): string {
  return (g.na ? g.na(rom) : '') || (rom.ported && !rom.ported.groups.includes(g.key) ? NOT_PORTED : '');
}

/** An option (`group.opt`) or a choice of one (`group.opt#index`) the new engine doesn't do yet. */
const notPorted = (rom: RomInfo, key: string) => !!rom.ported?.off.includes(key);
const isControl = (o: OptDef) => o.k !== 'sub' && o.k !== 'info' && o.k !== 'update';

const BATTLE_STYLES = ['Single Battles', 'Double Battles', 'Triple Battles', 'Rotation Battles'];

export function choiceList(o: OptDef, rom: RomInfo): readonly string[] {
  if (o.choices === 'MDGENS') {
    const out: string[] = [];
    for (let g = rom.genNum + 1; g <= 9; g++) out.push('Gen ' + g);
    return out;
  }
  if (o.choices === 'PBSGENS') {
    const out: string[] = [];
    for (let g = Math.max(6, rom.genNum + 1); g <= 9 && out.length < 4; g++) out.push('Gen ' + g);
    return out;
  }
  if (o.choices === 'TYPES') return rom.genNum >= 6 ? [...TYPES, 'Fairy'] : TYPES;
  // Triple and rotation battles exist in Gen 5 and 6 only.
  if (o.choices === 'BATTLESTYLES') return BATTLE_STYLES.slice(0, rom.genNum === 5 || rom.genNum === 6 ? 4 : 2);
  return o.choices ?? [];
}

export interface OptState {
  hidden: boolean;
  disabled: boolean;
  choiceOk: ((ci: number) => boolean) | null;
}

export interface Evaluation {
  /** Effective values: `group` → mode, `group.opt.on` → boolean, `group.opt.v` → value. */
  val: Record<string, unknown>;
  state: Record<string, OptState>;
  ctx: Ctx;
}

/**
 * Applies every enable/visibility rule until the state is stable; it runs again after each change.
 */
function evaluate(secs: SectionDef[], rom: RomInfo, ov: Overrides): Evaluation {
  const get = (k: string, d: unknown) => (ov[k] === undefined ? d : ov[k]);
  const groups = secs.flatMap((s) => s.groups);
  const val: Record<string, unknown> = {};
  const state: Record<string, OptState> = {};
  const def = (o: OptDef) => (o.k === 'species' ? rom.starters[o.slot ?? 0] : o.val);
  const ctx = new RuleContext(val, rom);
  for (const g of groups) {
    if (g.modes) val[g.key] = get(g.key, g.mi);
    // The chosen graphics pack: one of the game's, the first by default.
    if (g.cpg) {
      const packs = rom.cpgPacks.map((p) => p.pack.id);
      const picked = get(g.key + '.pack', null);
      val[g.key + '.pack'] = packs.includes(picked as string) ? picked : (packs[0] ?? null);
    }
    // Misc tweaks: on when chosen, the game has them and the group is ported.
    if (g.tweaks) for (const id of rom.tweaks) val[g.key + '.' + id] = !groupNa(g, rom) && !!get(g.key + '.' + id, false);
    g.opts.forEach((o, oi) => {
      const k = optKey(g, o, oi);
      val[k + '.on'] = !!get(k + '.on', o.on);
      val[k + '.v'] = get(k + '.v', def(o));
    });
  }
  for (let pass = 0; pass < 4; pass++) {
    for (const g of groups) {
      const gHidden = !!(g.vis && !g.vis(rom));
      const gNa = groupNa(g, rom);
      if (g.modes) {
        const modeOk = (i: number) => !gHidden && !gNa && !notPorted(rom, g.key + '#' + i) && !(g.mvis && !g.mvis(rom, i)) && !(g.men && !g.men(ctx, i));
        let m = get(g.key, g.mi) as number;
        if (!modeOk(m)) m = g.mfall !== undefined && modeOk(g.mfall) ? g.mfall : 0;
        val[g.key] = m;
      }
      const mode = g.modes ? (val[g.key] as number) : 0;
      const inactive = !!g.modes && mode === 0;
      g.opts.forEach((o, oi) => {
        if (!isControl(o)) return;
        const k = optKey(g, o, oi);
        const hidden = gHidden || !!gNa || !!(o.vis && !o.vis(rom));
        const disabled = hidden || notPorted(rom, k) || (o.need ? !o.need.includes(mode) : inactive && !o.free) || !!(o.en && !o.en(ctx));
        val[k + '.on'] = disabled ? !hidden && !!o.disOn : !!get(k + '.on', o.on);
        let v = get(k + '.v', def(o));
        let choiceOk: OptState['choiceOk'] = null;
        if (o.k === 'radio') {
          choiceOk = (ci) => !notPorted(rom, k + '#' + ci) && !(o.cvis && !o.cvis(rom, ci)) && !(o.cen && !o.cen(ctx, ci));
          if (disabled || !choiceOk(v as number)) v = o.def ?? 0;
        }
        val[k + '.v'] = v;
        state[k] = { hidden, disabled, choiceOk };
      });
    }
  }
  return { val, state, ctx };
}

// ---------- View model ----------

export interface ChoiceView { i: number; label: string; disabled: boolean; cur: boolean }
export interface OptView {
  kind: OptDef['k'];
  key: string;
  label: string;
  text: string;
  tip: string;
  disabled: boolean;
  on: boolean;
  value: number | string;
  hasToggle: boolean;
  min?: number;
  max?: number;
  valueText: string;
  choices: ChoiceView[];
  choiceTip: string;
  dexNum: string;
  invalid: boolean;
}
export interface TierCell { key: string; on: boolean; value: number; disabled: boolean; num: boolean; aria: string }
export interface MatrixRow { label: string; tip: string; cells: TierCell[] }
export interface TweakView { key: string; label: string; on: boolean; tip: string }
export interface GroupView {
  key: string;
  title: string;
  tag: string;
  note: string;
  unsupported: boolean;
  modes: ChoiceView[];
  mode: number;
  modeTip: string;
  opts: OptView[];
  changed: number;
  wide: boolean;
  matrix: MatrixRow[] | null;
  cpg: boolean;
  tweaks: TweakView[] | null;
}
export interface SectionView {
  id: string;
  label: string;
  desc: string;
  icon: string;
  groups: GroupView[];
  count: number;
  unsupported: boolean;
  keys: string[];
}
export interface SearchHit { label: string; where: string; section: string }

export interface View {
  sections: SectionView[];
  hits: SearchHit[];
  total: number;
  badStarter: boolean;
  val: Record<string, unknown>;
}

const TIERS = ['Boss', 'Important', 'Regular'];
const signed = (o: OptDef, v: unknown) => (o.k === 'slider' && (o.min ?? 0) < 0 && Number(v) > 0 ? '+' : '');

function buildView(secs: SectionDef[], rom: RomInfo, ov: Overrides, query: string): View {
  const { val, state, ctx } = evaluate(secs, rom, ov);
  const dex = DEX.slice(0, rom.dexSize);
  const dexSet = new Set(dex.map((d) => d.toLowerCase()).concat('random'));
  const q = query.trim().toLowerCase();
  const hits: SearchHit[] = [];
  let total = 0;
  let badStarter = false;

  const sections = secs.map((sec): SectionView => {
    let count = 0;
    const groups = sec.groups
      .filter((g) => !(g.vis && !g.vis(rom)))
      .map((g): GroupView => {
        const naText = groupNa(g, rom);
        const mode = g.modes ? (val[g.key] as number) : 0;
        let changed = g.modes && mode > 0 ? 1 : 0;
        const hit = (label: string, where = sec.label) => {
          if (q && label.toLowerCase().includes(q)) hits.push({ label, where, section: sec.id });
        };
        hit(g.title);
        const modes: ChoiceView[] = [];
        (g.modes ?? []).forEach((label, i) => {
          if (g.mvis && !g.mvis(rom, i)) return;
          if (i > 0) hit(label, sec.label + ' › ' + g.title);
          modes.push({ i, label, cur: i === mode, disabled: !!naText || notPorted(rom, g.key + '#' + i) || !!(g.men && !g.men(ctx, i)) });
        });
        const opts: OptView[] = [];
        const tiers: Record<string, TierCell> = {};
        g.opts.forEach((o, oi) => {
          const k = optKey(g, o, oi);
          const s = state[k] ?? { hidden: !!naText, disabled: false, choiceOk: null };
          if (s.hidden) return;
          const on = !!val[k + '.on'];
          const value = val[k + '.v'] as number | string;
          if (o.k === 'tier') {
            if (!s.disabled && on) changed++;
            tiers[o.row + '.' + o.col] = { key: k, on, value: Number(value), disabled: s.disabled, num: !!o.num, aria: '' };
            return;
          }
          if (isControl(o)) hit(o.label, sec.label + ' › ' + g.title);
          let invalid = false;
          if (o.k === 'species') {
            invalid = !s.disabled && !dexSet.has(String(value).trim().toLowerCase());
            if (invalid) badStarter = true;
          }
          if (!s.disabled && (o.k === 'toggle' || o.k === 'limit' || o.tog) && on !== !!o.on) changed++;
          else if (!s.disabled && o.k === 'radio' && value !== (o.val ?? 0)) changed++;
          else if (!s.disabled && o.k === 'species' && String(value) !== rom.starters[o.slot ?? 0]) changed++;
          const list = choiceList(o, rom);
          let text = o.label;
          if (o.k === 'info' && o.label === 'evo') {
            const evoOn = !!val['tp.evo.on'] && !state['tp.evo']?.disabled;
            const mod = Number(val['tp.evo.v'] ?? 0);
            text = 'Latest fully evolved level: ' + (evoOn ? Math.max(1, Math.round((55 * 100) / (100 + mod))) : '--');
          }
          const di = dex.indexOf(String(value));
          opts.push({
            kind: o.k,
            key: k,
            label: o.label,
            text,
            tip: isControl(o) ? tip(o.label, g.pfx, o.t) : '',
            disabled: s.disabled,
            on,
            value,
            hasToggle: !!o.tog,
            min: o.min,
            max: o.max,
            valueText: (o.unitText ?? '') + signed(o, value) + value + (o.unit ?? ''),
            choices: list
              .map((label, i) => ({ i, label, cur: i === value, disabled: s.disabled || !!(s.choiceOk && !s.choiceOk(i)) }))
              .filter((c) => !(o.cvis && !o.cvis(rom, c.i))),
            choiceTip: o.k === 'radio' ? (o.tips ? tip('', undefined, o.tips[value as number]) : tip(list[value as number], o.pfx ?? g.pfx)) : '',
            dexNum: di >= 0 ? '#' + String(di + 1).padStart(3, '0') : '',
            invalid
          });
        });
        const matrix = g.matrix
          ? g.matrix.map((label, ri) => ({
              label,
              tip: tip('', undefined, g.matrixTips?.[ri]),
              cells: [0, 1, 2].map((ci) => ({ ...(tiers[ri + '.' + ci] ?? { key: '', on: false, value: 1, disabled: true, num: false }), aria: label + ', ' + TIERS[ci] + ' trainers' }))
            }))
          : null;
        let tweaks: TweakView[] | null = null;
        if (g.tweaks && !naText) {
          tweaks = TWEAKS.filter(([id]) => rom.tweaks.includes(id)).map(([id, label]) => {
            const key = g.key + '.' + id;
            const on = !!ov[key];
            if (on) changed++;
            hit(label);
            return { key, label, on, tip: tip('', undefined, id) };
          });
        }
        const note = naText || (g.note ? g.note(rom) : '') || (tweaks && !tweaks.length ? 'There are no tweaks available for the currently loaded game.' : '');
        count += changed;
        return {
          key: g.key,
          title: g.title,
          tag: g.tag ?? '',
          note,
          unsupported: !!naText,
          modes,
          mode,
          modeTip: g.modes ? (g.tips ? tip('', undefined, g.tips[mode]) : tip(g.modes[mode], g.pfx)) : '',
          opts,
          changed,
          wide: !!g.wide,
          matrix,
          cpg: !!g.cpg && !naText,
          tweaks
        };
      });
    total += count;
    const unsupported = !!sec.unsupported?.(rom) || groups.every((g) => g.unsupported);
    return { id: sec.id, label: sec.label, desc: sec.desc, icon: sec.icon, groups, count, unsupported, keys: sec.groups.map((g) => g.key) };
  });
  return { sections, hits, total, badStarter, val };
}

/** Rule context over the live value map; enable rules only see effective values. */
export class RuleContext implements Ctx {
  constructor(
    private readonly val: Record<string, unknown>,
    readonly r: RomInfo
  ) {}
  m(k: string) {
    return (this.val[k] as number) || 0;
  }
  on(k: string) {
    return !!this.val[k + '.on'];
  }
  v(k: string) {
    return this.val[k + '.v'];
  }
  el() {
    return this.val.pe === 2;
  }
  megaOk() {
    return !(this.el() && !this.on('gen.irr') && this.on('pe.alt'));
  }
  spCustomRandom() {
    return this.m('sp') === 1 && ['s1', 's2', 's3'].some((s) => String(this.v('sp.' + s) ?? '').trim().toLowerCase() === 'random');
  }
  spBranch() {
    return !(this.m('sp') === 0 || (this.m('sp') === 1 && !this.spCustomRandom()));
  }
  tpAdded() {
    return [0, 1, 2].some((c) => this.on('tp.t0' + c));
  }
  tpItems() {
    return [0, 1, 2].some((c) => this.on('tp.t1' + c));
  }
  tpDiv(col: number) {
    const tp = this.m('tp');
    return tp === 0 ? this.on('tp.t0' + col) : tp !== 4 && tp !== 7;
  }
}

/** Facade over the option definitions: evaluates rules and builds the view model for the UI. */
export class OptionEngine {
  constructor(private readonly defs: SectionDef[]) {}

  evaluate(rom: RomInfo, ov: Overrides): Evaluation {
    return evaluate(this.defs, rom, ov);
  }

  view(rom: RomInfo, ov: Overrides, query = ''): View {
    return buildView(this.defs, rom, ov, query);
  }
}
