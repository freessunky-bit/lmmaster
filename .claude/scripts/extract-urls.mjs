import fs from 'node:fs';
import path from 'node:path';

const dir = 'manifests/snapshot/models';
const cats = fs.readdirSync(dir);
const urls = [];
for (const cat of cats) {
  const catDir = path.join(dir, cat);
  if (!fs.statSync(catDir).isDirectory()) continue;
  const files = fs.readdirSync(catDir).filter(f => f.endsWith('.json'));
  for (const f of files) {
    const full = path.join(catDir, f);
    const m = JSON.parse(fs.readFileSync(full, 'utf8'));
    for (const e of (m.entries || [])) {
      const id = e.id;
      const src = e.source || {};
      if (src.type === 'direct-url' && src.url) {
        urls.push({ manifest: full, id, kind: 'main', url: src.url });
      } else if (src.type === 'hugging-face') {
        const repo = src.repo;
        const qopts = e.quantization_options || [];
        if (qopts.length > 0) {
          for (const q of qopts) {
            const fp = q.file_path || src.file;
            if (fp) {
              const u = `https://huggingface.co/${repo}/resolve/main/${fp}`;
              urls.push({ manifest: full, id, kind: `main:${q.label}`, url: u });
            }
          }
        } else if (src.file) {
          const u = `https://huggingface.co/${repo}/resolve/main/${src.file}`;
          urls.push({ manifest: full, id, kind: 'main', url: u });
        }
      }
      if (e.mmproj && e.mmproj.url) {
        urls.push({ manifest: full, id, kind: 'mmproj', url: e.mmproj.url });
      }
    }
  }
}
const lines = urls.map(u => [u.id, u.kind, u.url, u.manifest.replaceAll('\\', '/')].join('\t'));
process.stdout.write(lines.join('\n') + '\n');
