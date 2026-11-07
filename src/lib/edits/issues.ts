// How validation messages are shown: per object, as links to the object, and as the reason saving is blocked.
// The rules themselves are Rust's (ADR 0008); this only arranges the issues the backend reports.
import type { EditIssue, EditSeverity } from '../backend';
import type { ObjectRef } from '../generated/ObjectRef';
import { EDIT_SECTIONS, refKey, sameRef, sectionOf, type EditSection, type SectionId } from './sections';

/** The section of issues about no one object, by their field: a limit the objects of one section share. */
const SHARED_LIMITS: Record<string, SectionId> = { 'shop space': 'shops' };

/** The section an issue belongs to; `null` for one about the list as a whole. */
export function issueSection(issue: EditIssue): EditSection | null {
  if (issue.target) return sectionOf(issue.target.kind);
  const id = issue.field ? SHARED_LIMITS[issue.field] : undefined;
  return EDIT_SECTIONS.find((s) => s.id === id) ?? null;
}

/** A message with a link to the object it is about, or to its section for a shared limit. */
export interface IssueLink {
  key: string;
  text: string;
  target: ObjectRef | null;
  severity: EditSeverity;
  /** `null` for an issue about the whole list, which has no link. */
  section: SectionId | null;
  /** The link's label, e.g. "Open starter", "Open shops"; `''` without a link. */
  goLabel: string;
}

/** The messages of one severity as links, in section order and, within a section, in the order Rust gave them. */
export function issueLinks(issues: readonly EditIssue[], severity: EditSeverity = 'error'): IssueLink[] {
  const order = (i: EditIssue) => {
    const s = issueSection(i);
    return s ? EDIT_SECTIONS.indexOf(s) : -1;
  };
  return issues
    .map((issue, n) => ({ issue, n }))
    .filter(({ issue }) => issue.severity === severity)
    .sort((a, b) => order(a.issue) - order(b.issue) || a.n - b.n)
    .map(({ issue, n }) => {
      const section = issueSection(issue);
      const goLabel = !section ? '' : issue.target ? 'Open ' + section.noun : 'Open ' + section.label.toLowerCase();
      const key = (issue.target ? refKey(issue.target) : 'list:' + (issue.field ?? '')) + '#' + n;
      return { key, text: issue.message, target: issue.target, severity, section: section?.id ?? null, goLabel };
    });
}

/** The issues about one object, errors first. */
export function issuesOf(issues: readonly EditIssue[], ref: ObjectRef): EditIssue[] {
  const mine = issues.filter((i) => i.target !== null && sameRef(i.target, ref));
  return [...mine.filter((i) => i.severity === 'error'), ...mine.filter((i) => i.severity !== 'error')];
}

/** The issues about a limit the objects of one section share, such as the shop space. */
export function sharedIssues(issues: readonly EditIssue[], section: SectionId): EditIssue[] {
  return issues.filter((i) => !i.target && issueSection(i)?.id === section);
}

/** How many errors are about one section: about its objects or a limit they share. */
export function errorCount(issues: readonly EditIssue[], section: SectionId): number {
  return issues.filter((i) => i.severity === 'error' && issueSection(i)?.id === section).length;
}

/** Why saving is blocked, or `''` when it isn't: errors block, warnings don't. */
export function blockText(issues: readonly EditIssue[]): string {
  const n = issues.filter((i) => i.severity === 'error').length;
  return n ? 'Manual edits: fix ' + (n === 1 ? '1 problem' : n + ' problems') + ' before saving.' : '';
}
