// The command line typed on a control surface keypad, Onyx style:
// "1 Thru 4 - 2 @ 5", "Group 3 @ Full", "Cue 7".

export type Token = "Thru" | "+" | "-" | "@" | "Full" | "Group" | "Cue" | string;

export type Command =
  | { kind: "fixtures"; fixtures: number[] | null; level: number | null }
  | { kind: "groups"; groups: number[]; level: number | null }
  | { kind: "cue"; cue: number };

const WORDS = new Set(["Thru", "+", "-", "@", "Full", "Group", "Cue"]);
const isNumber = (t: Token | undefined): t is string => t != null && !WORDS.has(t);

/** Adds a keypad digit or "." to the line, joining it to a number being typed. */
export function typeDigit(line: Token[], digit: string): Token[] {
  const last = line[line.length - 1];
  if (isNumber(last)) {
    if (digit === "." && last.includes(".")) return line;
    return [...line.slice(0, -1), last + digit];
  }
  return [...line, digit];
}

/** Takes back the last key: one digit of a number, or a whole word. */
export function backspace(line: Token[]): Token[] {
  const last = line[line.length - 1];
  if (isNumber(last) && last.length > 1) return [...line.slice(0, -1), last.slice(0, -1)];
  return line.slice(0, -1);
}

/**
 * A level after @ from 0 to 1. One digit is tens, as on Onyx: "@ 5" is 50%,
 * "@ 05" is 5% and "@ 55" is 55%.
 */
function level(token: Token | undefined): number | null {
  if (token === "Full") return 1;
  if (!isNumber(token)) return null;
  const n = Number(token);
  if (!Number.isFinite(n)) return null;
  const percent = token.length === 1 ? n * 10 : n;
  return Math.min(100, Math.max(0, percent)) / 100;
}

/** Numbers joined by Thru, + and -, as a list of ids taken from `known`. */
function ids(tokens: Token[], known: number[]): number[] | string {
  const picked = new Set<number>();
  let remove = false;
  for (let i = 0; i < tokens.length; i++) {
    const t = tokens[i];
    if (t === "+" || t === "-") {
      remove = t === "-";
      continue;
    }
    if (!isNumber(t)) return `Unexpected ${t}`;
    const from = Number(t);
    let to = from;
    if (tokens[i + 1] === "Thru") {
      if (!isNumber(tokens[i + 2])) return "Thru needs a number after it";
      to = Number(tokens[i + 2]);
      i += 2;
    }
    const [lo, hi] = from <= to ? [from, to] : [to, from];
    for (const id of known) if (id >= lo && id <= hi) (remove ? picked.delete(id) : picked.add(id));
  }
  return [...picked];
}

/** Reads a finished command line, or says what's wrong with it. */
export function parse(line: Token[], fixtureIds: number[], groupIds: number[]): Command | string {
  if (line.length === 0) return "Nothing typed";
  const at = line.indexOf("@");
  const target = at >= 0 ? line.slice(0, at) : line;
  let lvl: number | null = null;
  if (at >= 0) {
    if (line.length !== at + 2 || (lvl = level(line[at + 1])) == null) return "@ needs a level, such as @ 5 or @ Full";
  }

  if (target[0] === "Cue") {
    if (at >= 0 || target.length !== 2 || !isNumber(target[1])) return "Type Cue, a number, then Enter";
    return { kind: "cue", cue: Number(target[1]) };
  }
  if (target[0] === "Group") {
    const groups = ids(target.slice(1), groupIds);
    if (typeof groups === "string") return groups;
    if (groups.length === 0) return "No such group";
    return { kind: "groups", groups, level: lvl };
  }
  if (target.length === 0) {
    if (lvl == null) return "Nothing to do";
    return { kind: "fixtures", fixtures: null, level: lvl };
  }
  const fixtures = ids(target, fixtureIds);
  if (typeof fixtures === "string") return fixtures;
  if (fixtures.length === 0) return "No such fixture";
  return { kind: "fixtures", fixtures, level: lvl };
}
