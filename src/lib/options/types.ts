import type { RomInfo } from '../data/roms';

/** Read-only view on the evaluated option state, used by enable rules. */
export interface Ctx {
  r: RomInfo;
  /** Effective mode index of a group. */
  m(groupKey: string): number;
  /** Effective on/off of an option (`group.option`). */
  on(key: string): boolean;
  /** Effective value of an option (`group.option`). */
  v(key: string): unknown;
  /** Evolutions are set to "Random Every Level". */
  el(): boolean;
  megaOk(): boolean;
  spCustomRandom(): boolean;
  spBranch(): boolean;
  tpAdded(): boolean;
  tpItems(): boolean;
  tpDiv(col: number): boolean;
}

export type OptKind = 'toggle' | 'slider' | 'select' | 'number' | 'radio' | 'sub' | 'info' | 'update' | 'limit' | 'species' | 'tier';

/** Dynamic choice lists resolved per ROM. */
export type ChoiceSource = 'MDGENS' | 'PBSGENS' | 'TYPES' | 'BATTLESTYLES';

export interface OptDef {
  k: OptKind;
  label: string;
  /** Short key, unique within its group. Full key is `group.key`. */
  key?: string;
  /** Default on/off for toggles and for options with their own checkbox. */
  on?: boolean;
  /** Default value (slider/number/select/radio index, species name). */
  val?: number | string;
  min?: number;
  max?: number;
  unit?: string;
  unitText?: string;
  choices?: readonly string[] | ChoiceSource;
  /** Available regardless of the group mode (otherwise disabled in mode 0). */
  free?: boolean;
  /** Only enabled in these group modes. */
  need?: number[];
  /** Extra enable rule. */
  en?: (c: Ctx) => boolean;
  /** Visibility for the loaded ROM. */
  vis?: (r: RomInfo) => boolean;
  /** Tooltip key: the control's name in `tips.json`. */
  t?: string;
  /** Slider/select/number that has its own checkbox. */
  tog?: boolean;
  /** Per-choice enable rule (radio). */
  cen?: (c: Ctx, ci: number) => boolean;
  /** Per-choice visibility (radio). */
  cvis?: (r: RomInfo, ci: number) => boolean;
  /** Value a radio falls back to when it is disabled. */
  def?: number;
  /** Shown checked while disabled. */
  disOn?: boolean;
  /** Tooltip keys per radio choice. */
  tips?: string[];
  pfx?: string[];
  slot?: number;
  row?: number;
  col?: number;
  num?: boolean;
}

export interface GroupDef {
  title: string;
  key: string;
  modes: readonly string[] | null;
  mi: number;
  opts: OptDef[];
  pfx?: string[];
  wide?: boolean;
  tag?: string;
  vis?: (r: RomInfo) => boolean;
  /** Returns a "not supported" message when the ROM can't use this group. */
  na?: (r: RomInfo) => string;
  note?: (r: RomInfo) => string;
  /** Per-mode enable rule. */
  men?: (c: Ctx, i: number) => boolean;
  /** Per-mode visibility. */
  mvis?: (r: RomInfo, i: number) => boolean;
  /** Mode to fall back to when the selected one becomes unavailable. */
  mfall?: number;
  /** Tooltip keys per mode. */
  tips?: string[];
  matrix?: string[];
  matrixTips?: string[];
  cpg?: boolean;
  tweaks?: boolean;
}

export interface SectionDef {
  id: string;
  label: string;
  icon: string;
  desc: string;
  groups: GroupDef[];
  unsupported?: (r: RomInfo) => boolean;
}

/** Raw user choices, keyed by `group` (mode) or `group.option.on` / `group.option.v`. */
export type Overrides = Record<string, unknown>;
