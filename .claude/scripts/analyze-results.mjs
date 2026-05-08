import fs from 'node:fs';

const lines = fs.readFileSync('.claude/scripts/url-check-results.tsv', 'utf8').trim().split('\n');
// id, kind, code, url, manifest
const rows = lines.map(l => {
  const [id, kind, code, url, manifest] = l.split('\t');
  return { id, kind, code, url, manifest };
});

// Group by manifest
const byManifest = new Map();
for (const r of rows) {
  if (!byManifest.has(r.manifest)) byManifest.set(r.manifest, []);
  byManifest.get(r.manifest).push(r);
}

const okManifests = [];
const failManifests = [];

for (const [m, rs] of byManifest) {
  const fails = rs.filter(r => r.code !== '200' && r.code !== '301' && r.code !== '302');
  if (fails.length === 0) {
    okManifests.push({ manifest: m, id: rs[0].id });
  } else {
    failManifests.push({ manifest: m, id: rs[0].id, fails, total: rs.length });
  }
}

console.log('=== OK MANIFESTS (' + okManifests.length + ') ===');
for (const o of okManifests.sort((a, b) => a.manifest.localeCompare(b.manifest))) {
  console.log(`  ${o.id.padEnd(34)}  ${o.manifest}`);
}

console.log('\n=== FAIL MANIFESTS (' + failManifests.length + ') ===');
for (const f of failManifests.sort((a, b) => a.manifest.localeCompare(b.manifest))) {
  console.log(`\n[${f.id}] ${f.manifest}`);
  console.log(`  total quants: ${f.total}, failures: ${f.fails.length}`);
  for (const x of f.fails) {
    console.log(`  ${x.code} ${x.kind.padEnd(14)} ${x.url}`);
  }
}

console.log('\n=== TOTAL ===');
console.log(`  OK manifests:   ${okManifests.length}`);
console.log(`  FAIL manifests: ${failManifests.length}`);
console.log(`  Total URLs:     ${rows.length}`);
console.log(`  Codes: 200=${rows.filter(r => r.code === '200').length} 401=${rows.filter(r => r.code === '401').length} 404=${rows.filter(r => r.code === '404').length} other=${rows.filter(r => !['200', '401', '404'].includes(r.code)).length}`);
