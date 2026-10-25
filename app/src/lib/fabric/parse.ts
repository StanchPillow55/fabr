import { FIELDS, OPTIONAL_FIELDS, SECTION_HEADINGS, STATES, STEPS, type FrontMatter, type LogEntry, type Sections, type TaskRecord } from './types';
import { parseYaml } from './yaml';

export function parseFrontMatter(text: string): { data: Record<string, unknown>; body: string } {
  const lines = text.replace(/\r\n/g, '\n').split('\n');
  if (lines[0] !== '---') throw new Error('record must start with a "---" front matter line');
  const end = lines.indexOf('---', 1);
  if (end < 0) throw new Error('front matter is not closed by a "---" line');
  return { data: parseYaml(lines.slice(1, end).join('\n')), body: lines.slice(end + 1).join('\n') };
}

const LOG_LINE = /^- (\S+) ([a-z]+): (.*)$/;

export function parseLogLine(line: string): LogEntry | null {
  const m = LOG_LINE.exec(line.trim());
  return m ? { ts: m[1], step: m[2], message: m[3] } : null;
}

export function parseSections(body: string): Sections {
  const lines = body.split('\n');
  const found: { name: string; at: number }[] = [];
  let next = 0;
  lines.forEach((l, i) => {
    if (next < SECTION_HEADINGS.length && l.trimEnd() === `## ${SECTION_HEADINGS[next]}`) found.push({ name: SECTION_HEADINGS[next++], at: i });
  });
  if (found.length < SECTION_HEADINGS.length) {
    throw new Error(`missing section heading "## ${SECTION_HEADINGS[found.length]}" (headings must be ${SECTION_HEADINGS.map((h) => `## ${h}`).join(', ')} in this order)`);
  }
  const text = (k: number) => lines.slice(found[k].at + 1, k + 1 < found.length ? found[k + 1].at : undefined).join('\n').trim();
  const log = text(4);
  const logEntries = log.split('\n').map(parseLogLine).filter((e): e is LogEntry => !!e);
  return { context: text(0), spec: text(1), success: text(2), plan: text(3), log, logEntries };
}

export function splitContext(context: string): { own: string; gathered: string } {
  const i = context.split('\n').findIndex((l) => l.trimEnd() === '### Gathered');
  if (i < 0) return { own: context, gathered: '' };
  const lines = context.split('\n');
  return { own: lines.slice(0, i).join('\n').trim(), gathered: lines.slice(i + 1).join('\n').trim() };
}

export function parseRecord(text: string): TaskRecord {
  const { data, body } = parseFrontMatter(text);
  if (data.schema !== 2 && data.schema !== 3) throw new Error(`unsupported schema ${String(data.schema)}`);
  for (const f of FIELDS) if (!(f in data) && !OPTIONAL_FIELDS.includes(f)) throw new Error(`missing front matter field "${f}"`);
  if (!(STATES as readonly string[]).includes(data.state as string)) throw new Error(`invalid state "${String(data.state)}"`);
  if (data.step !== null && !(STEPS as readonly string[]).includes(data.step as string)) throw new Error(`invalid step "${String(data.step)}"`);
  for (const f of ['requests', 'grants', 'links'] as const) {
    if (typeof data[f] !== 'object' || data[f] === null) throw new Error(`front matter field "${f}" must be a map`);
  }
  for (const f of ['answers', 'assertions'] as const) {
    if (!Array.isArray(data[f])) throw new Error(`front matter field "${f}" must be a list`);
  }
  const pri = data.pri ?? 0;
  if (typeof pri !== 'number' || !Number.isInteger(pri) || pri < 0 || pri > 3) throw new Error(`invalid pri "${String(pri)}"`);
  return { ...(data as unknown as FrontMatter), pri, sections: parseSections(body) };
}
