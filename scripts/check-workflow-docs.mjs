import { readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// Mechanical half of P66.01. Content review remains a separate, human-readable
// comparison with the approved behavior; matching words cannot prove it.
const root = path.resolve(process.argv[2] ?? fileURLToPath(new URL('../', import.meta.url)));
const entries = ['README.md', 'docs/workflow-contract.md', 'docs/workflow-runtime.md',
  'docs/workflow-fan-in-acceptance.md', 'docs/scheduler.md', 'docs/usage.md',
  'docs/example/README.md', 'GLOSSARY.md'];
const visited = new Set();
const errors = [];
let links = 0;

function prose(text) {
  return text.replace(/^(`{3,}|~{3,})[^\n]*\n[\s\S]*?^\1[^\n]*$/gm, '');
}
function anchors(text) {
  const seen = new Map();
  const result = new Set();
  for (const match of prose(text).matchAll(/^#{1,6}\s+(.+?)\s*#*$/gm)) {
    const base = match[1].replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
      .replace(/<[^>]+>/g, '').toLowerCase().replace(/[^\p{L}\p{M}\p{N}_\-\s]/gu, '').replace(/\s/g, '-');
    const count = seen.get(base) ?? 0;
    seen.set(base, count + 1);
    result.add(count ? `${base}-${count}` : base);
  }
  for (const match of prose(text).matchAll(/<(?:a|h[1-6])\b[^>]*(?:id|name)=["']([^"']+)["']/gi)) result.add(match[1]);
  return result;
}
async function inspect(relative) {
  if (visited.has(relative)) return;
  visited.add(relative);
  let text;
  try { text = await readFile(path.join(root, relative), 'utf8'); }
  catch (error) { errors.push(`${relative}: ${error.code}`); return; }
  const references = new Map([...prose(text).matchAll(/^\s*\[([^\]]+)\]:\s*<?([^\s>]+)>?/gm)]
    .map(match => [match[1].toLowerCase(), match[2]]));
  const destinations = [...prose(text).matchAll(/!?\[[^\]\n]*\]\(<?([^\s)>]+)>?(?:\s+["'][^\n]*?["'])?\)/g)].map(match => match[1]);
  for (const match of prose(text).matchAll(/!?\[([^\]\n]+)\]\[([^\]\n]*)\]/g)) {
    const target = references.get((match[2] || match[1]).toLowerCase());
    if (target) destinations.push(target);
    else errors.push(`${relative}: undefined reference ${match[0]}`);
  }
  for (const destination of destinations) {
    if (/^(?:https?:|mailto:|data:)/i.test(destination)) continue;
    links++;
    const [name, fragment] = destination.split('#');
    const target = path.resolve(root, path.dirname(relative), decodeURIComponent(name));
    const local = path.relative(root, target).replaceAll('\\', '/');
    if (local.startsWith('../') || path.isAbsolute(local) || local.split('/').some(part => part === '.scratch' || part === '.git')) {
      errors.push(`${relative}: non-durable dependency ${destination}`);
      continue;
    }
    try {
      const info = await stat(target);
      if (fragment && info.isFile()) {
        const body = await readFile(target, 'utf8');
        if (!anchors(body).has(decodeURIComponent(fragment))) errors.push(`${relative}: missing anchor ${destination}`);
      }
      if (info.isFile() && target.endsWith('.md')) await inspect(local);
    } catch (error) { errors.push(`${relative}: broken link ${destination} (${error.code})`); }
  }
}
for (const entry of entries) await inspect(entry);
console.log(JSON.stringify({ root, documents: visited.size, localLinks: links, errors }, null, 2));
if (errors.length) process.exitCode = 1;
