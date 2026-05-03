// Version bump — 한 명령으로 모든 동기 + commit + tag.
//
// 사용법:
//   node .claude/scripts/bump-version.mjs 0.0.2
//   node .claude/scripts/bump-version.mjs 0.0.2 --dry-run   # 변경 미리보기
//   node .claude/scripts/bump-version.mjs 0.0.2 --no-commit # 파일만 수정 (commit/tag X)
//
// 동작:
// 1. 인자 검증 (semver MAJOR.MINOR.PATCH)
// 2. tauri.conf.json::version 갱신
// 3. Cargo.toml::workspace.package.version 갱신
// 4. apps/desktop/package.json::version 갱신
// 5. root package.json::version 갱신
// 6. git add + commit "chore: v{version}" + tag v{version}
// 7. 출력 — 다음 단계 안내 (push origin main + push origin v{version})

import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repo = path.resolve(__dirname, "..", "..");

const args = process.argv.slice(2);
const versionArg = args.find((a) => /^\d+\.\d+\.\d+/.test(a));
const dryRun = args.includes("--dry-run");
const noCommit = args.includes("--no-commit");

if (!versionArg) {
  console.error("사용법: node .claude/scripts/bump-version.mjs <version> [--dry-run] [--no-commit]");
  console.error("  예시: node .claude/scripts/bump-version.mjs 0.0.2");
  process.exit(1);
}

if (!/^\d+\.\d+\.\d+(-[a-z0-9.]+)?$/i.test(versionArg)) {
  console.error(`semver(MAJOR.MINOR.PATCH 또는 MAJOR.MINOR.PATCH-pre)가 아니에요: ${versionArg}`);
  process.exit(1);
}

const newVersion = versionArg;

const tauriConfPath = path.join(repo, "apps/desktop/src-tauri/tauri.conf.json");
const cargoTomlPath = path.join(repo, "Cargo.toml");
const desktopPkgPath = path.join(repo, "apps/desktop/package.json");
const rootPkgPath = path.join(repo, "package.json");

function readJson(p) {
  return JSON.parse(fs.readFileSync(p, "utf8"));
}

function writeJson(p, obj) {
  // 들여쓰기 2 + trailing newline (편집기 기본).
  const text = JSON.stringify(obj, null, 2) + "\n";
  if (dryRun) {
    console.log(`[dry-run] ${path.relative(repo, p)} 갱신 예정 (version=${newVersion})`);
    return;
  }
  fs.writeFileSync(p, text, "utf8");
  console.log(`✔ ${path.relative(repo, p)} (version=${newVersion})`);
}

function patchCargoToml() {
  const txt = fs.readFileSync(cargoTomlPath, "utf8");
  // [workspace.package] 블록의 version 라인만 정확히 갱신.
  // 보수적 정규식 — 다른 [package] 블록 영향 X.
  const reBlock = /(\[workspace\.package\][^\[]*?)(\nversion\s*=\s*")([^"]+)(")/s;
  const m = txt.match(reBlock);
  if (!m) {
    console.error("Cargo.toml [workspace.package] version 라인을 못 찾았어요.");
    process.exit(1);
  }
  const next = txt.replace(reBlock, `$1$2${newVersion}$4`);
  if (dryRun) {
    console.log(`[dry-run] Cargo.toml 갱신 예정 (workspace.package.version=${newVersion})`);
    return;
  }
  fs.writeFileSync(cargoTomlPath, next, "utf8");
  console.log(`✔ Cargo.toml (workspace.package.version=${newVersion})`);
}

// 1. tauri.conf.json
const tauriConf = readJson(tauriConfPath);
tauriConf.version = newVersion;
writeJson(tauriConfPath, tauriConf);

// 2. Cargo.toml workspace.package.version
patchCargoToml();

// 3. apps/desktop/package.json
const desktopPkg = readJson(desktopPkgPath);
desktopPkg.version = newVersion;
writeJson(desktopPkgPath, desktopPkg);

// 4. root package.json
const rootPkg = readJson(rootPkgPath);
rootPkg.version = newVersion;
writeJson(rootPkgPath, rootPkg);

// 5. commit + tag
if (noCommit || dryRun) {
  console.log("\n[--no-commit / --dry-run] commit + tag 단계 건너뜀.");
  if (!dryRun) {
    console.log("\n수동 진행:");
    console.log(`  git add -A`);
    console.log(`  git commit -m "chore: v${newVersion}"`);
    console.log(`  git tag v${newVersion}`);
  }
} else {
  try {
    execSync("git add -A", { cwd: repo, stdio: "inherit" });
    execSync(`git commit -m "chore: v${newVersion}"`, { cwd: repo, stdio: "inherit" });
    execSync(`git tag v${newVersion}`, { cwd: repo, stdio: "inherit" });
    console.log(`\n✔ commit + tag v${newVersion} 생성 완료`);
  } catch (e) {
    console.error("commit/tag 실패 — 수동 처리 필요.", e.message);
    process.exit(1);
  }
}

console.log("\n다음 단계 — release 빌드 trigger:");
console.log(`  git push origin main`);
console.log(`  git push origin v${newVersion}`);
console.log("\nGitHub Actions가 자동으로 Win/macOS/Linux 빌드 + minisign 서명 + Release(draft) 생성.");
console.log("빌드 끝나면 https://github.com/freessunky-bit/lmmaster/releases 에서 publish 클릭.");
