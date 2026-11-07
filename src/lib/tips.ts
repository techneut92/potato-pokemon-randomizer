import raw from './data/tips.json';

/** The tooltips: control name → [normalized label, text]. */
const TIPS = raw as unknown as Record<string, [string, string]>;
const cache = new Map<string, string>();

const norm = (s: string) => s.toLowerCase().replace(/é/g, 'e').replace(/[^a-z0-9]/g, '');

/**
 * Looks up a tooltip by control name (`key`), or else by label within the given
 * control-name prefixes (the character after the prefix must not be lowercase).
 */
export function tip(label: string | undefined, pfx?: string[], key?: string): string {
  if (key && TIPS[key]) return TIPS[key][1];
  if (!label) return '';
  const n = norm(label);
  const ck = (pfx ?? []).join('|') + '#' + n;
  const hit = cache.get(ck);
  if (hit !== undefined) return hit;
  let found = '';
  for (const p of pfx ?? ['']) {
    for (const k of Object.keys(TIPS)) {
      if (p && (!k.startsWith(p) || /[a-z]/.test(k.charAt(p.length)))) continue;
      if (TIPS[k][0] === n) {
        found = TIPS[k][1];
        break;
      }
    }
    if (found) break;
  }
  cache.set(ck, found);
  return found;
}
