import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join } from 'node:path';

const VERSION = '3.0.5';
const RELEASE_BASE = `https://github.com/fiso64/sockseek/releases/download/v${VERSION}`;

const targets = {
  'x86_64-unknown-linux-gnu': {
    asset: `sockseek_${VERSION}_linux-x64.tar.gz`,
    sha256: 'd0a1e909297bc4aa0e497bfdd7203884945a78adfb3c0477c8f22830ac951b66',
    executable: 'sockseek',
  },
  'aarch64-apple-darwin': {
    asset: `sockseek_${VERSION}_osx-arm64.tar.gz`,
    sha256: '6a18078b78f98a9797d723925574996a7bf87f6cc3012aee9efa1e9977a0edd0',
    executable: 'sockseek',
  },
  'x86_64-apple-darwin': {
    asset: `sockseek_${VERSION}_osx-x64.tar.gz`,
    sha256: 'dee6bbed20d2538dde7f4a854fc2ac400ad4a9adc5d75c2711aeecf37e7af1f5',
    executable: 'sockseek',
  },
  'x86_64-pc-windows-msvc': {
    asset: `sockseek_${VERSION}_win-x64.zip`,
    sha256: '1b5c1189dcfc24cc9fea22dc67a58b7a5f0a127d0367355cf2a8718100044802',
    executable: 'sockseek.exe',
  },
};

function rustHostTarget() {
  const output = execFileSync('rustc', ['-vV'], { encoding: 'utf8' });
  const host = output.match(/^host: (.+)$/m)?.[1];
  if (!host) throw new Error('Could not determine the Rust host target.');
  return host.trim();
}

const target =
  process.env.SOCKSEEK_TARGET || process.argv[2] || rustHostTarget();
const release = targets[target];
if (!release) {
  throw new Error(
    `Sockseek ${VERSION} is not configured for target ${target}.`,
  );
}

const destinationDir = join(process.cwd(), 'src-tauri', 'binaries');
const destination = join(
  destinationDir,
  `sockseek-${target}${target.includes('windows') ? '.exe' : ''}`,
);
if (existsSync(destination)) {
  console.log(`Sockseek sidecar already exists: ${destination}`);
  process.exit(0);
}

const temporary = mkdtempSync(join(tmpdir(), 'refrain-sockseek-'));
try {
  const archive = join(temporary, basename(release.asset));
  const response = await fetch(`${RELEASE_BASE}/${release.asset}`);
  if (!response.ok) {
    throw new Error(`Sockseek download failed with HTTP ${response.status}.`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  const digest = createHash('sha256').update(bytes).digest('hex');
  if (digest !== release.sha256) {
    throw new Error(
      `Sockseek checksum mismatch. Expected ${release.sha256}, received ${digest}.`,
    );
  }
  writeFileSync(archive, bytes);

  const extracted = join(temporary, 'extracted');
  mkdirSync(extracted);
  const extraction = spawnSync('tar', ['-xf', archive, '-C', extracted], {
    stdio: 'inherit',
  });
  if (extraction.status !== 0) {
    throw new Error(`Could not extract ${release.asset}.`);
  }

  const source = join(extracted, release.executable);
  if (!existsSync(source)) {
    throw new Error(`Sockseek archive did not contain ${release.executable}.`);
  }
  mkdirSync(destinationDir, { recursive: true });
  copyFileSync(source, destination);
  if (!target.includes('windows')) chmodSync(destination, 0o755);
  console.log(`Installed Sockseek ${VERSION} sidecar for ${target}.`);
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
