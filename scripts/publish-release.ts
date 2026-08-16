import { readFileSync, writeFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import { resolve, join, basename } from 'node:path';
import { S3Client, PutObjectCommand } from '@aws-sdk/client-s3';

// Parse command line arguments
// Usage: bun run release [--version 0.2.0] [--notes "Release changelog"] [--skip-build]
const args = process.argv.slice(2);
function getArg(flag: string, alias?: string): string | null {
  for (let i = 0; i < args.length; i++) {
    if (args[i] === flag || (alias && args[i] === alias)) {
      return args[i + 1] || null;
    }
    if (args[i].startsWith(`${flag}=`)) {
      return args[i].slice(`${flag}=`.length);
    }
  }
  return null;
}
const hasFlag = (flag: string) => args.includes(flag);

const requestedVersion = getArg('--version', '-v');
const requestedNotes = getArg('--notes', '-n') || process.env.RELEASE_NOTES;
const skipBuild = hasFlag('--skip-build');
const dryRun = hasFlag('--dry-run');

// 1. Read / Update version
const tauriConfPath = resolve('src-tauri/tauri.conf.json');
const pkgJsonPath = resolve('package.json');

if (!existsSync(tauriConfPath) || !existsSync(pkgJsonPath)) {
  console.error('❌ Could not find tauri.conf.json or package.json');
  process.exit(1);
}

const tauriConf = JSON.parse(readFileSync(tauriConfPath, 'utf8'));
const pkgJson = JSON.parse(readFileSync(pkgJsonPath, 'utf8'));

let version = requestedVersion || tauriConf.version || pkgJson.version || '0.1.0';

if (requestedVersion && requestedVersion !== tauriConf.version) {
  console.log(`📝 Bumping version from ${tauriConf.version} -> ${requestedVersion}...`);
  tauriConf.version = requestedVersion;
  pkgJson.version = requestedVersion;
  writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2), 'utf8');
  writeFileSync(pkgJsonPath, JSON.stringify(pkgJson, null, 2), 'utf8');
  version = requestedVersion;
}

const defaultNotes = `Phiên bản v${version}: Cập nhật tính năng và cải thiện hiệu năng cho RustSale CRM.`;
const notes = requestedNotes || defaultNotes;

const privateKeyPath = process.env.TAURI_SIGNING_PRIVATE_KEY_PATH || resolve(process.env.HOME || '~', '.tauri/rustsale.key');
const privateKeyPassword = process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD || '';
const publicUrl = process.env.R2_PUBLIC_URL || 'https://pub-6f14359d39ae4d50ac90b7e759b6de56.r2.dev';
const bucketName = process.env.R2_BUCKET_NAME || 'rustsale-releases';
const endpoint = process.env.R2_ENDPOINT || 'https://94af112d3d793eb10ab906f0bf8254cd.r2.cloudflarestorage.com';
const accessKeyId = process.env.R2_ACCESS_KEY_ID;
const secretAccessKey = process.env.R2_SECRET_ACCESS_KEY;

console.log(`\n======================================================`);
console.log(`🚀 RUSTSALE CRM RELEASE PIPELINE (v${version})`);
console.log(`======================================================`);
console.log(`📌 Version:      ${version}`);
console.log(`📝 Changelog:    ${notes}`);
console.log(`🔑 Private Key:  ${privateKeyPath}`);
console.log(`🌐 Public R2 URL:${publicUrl}`);
console.log(`🪣 R2 Bucket:    ${bucketName}`);
console.log(`======================================================\n`);

// 2. Build step
if (!skipBuild) {
  console.log(`🛠️ [1/4] Building Cloak runtime & frontend...`);
  const buildCloak = Bun.spawnSync(['bun', 'run', 'build:cloak-runtime'], { stdout: 'inherit', stderr: 'inherit' });
  if (!buildCloak.success) {
    console.error('❌ Failed to build cloak-runtime');
    process.exit(1);
  }

  const buildFe = Bun.spawnSync(['bun', 'run', 'build'], { stdout: 'inherit', stderr: 'inherit' });
  if (!buildFe.success) {
    console.error('❌ Failed to build frontend');
    process.exit(1);
  }

  console.log(`\n📦 [2/4] Building and signing Tauri bundle with Ed25519 key...`);
  const privateKeyContent = existsSync(privateKeyPath) ? readFileSync(privateKeyPath, 'utf8').trim() : (process.env.TAURI_SIGNING_PRIVATE_KEY || '');
  const envVars = {
    ...process.env,
    TAURI_SIGNING_PRIVATE_KEY: privateKeyContent,
    TAURI_PRIVATE_KEY: privateKeyContent,
    TAURI_SIGNING_PRIVATE_KEY_PATH: privateKeyPath,
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: privateKeyPassword,
    TAURI_KEY_PASSWORD: privateKeyPassword,
  };

  const tauriBuild = Bun.spawnSync(['bunx', 'tauri', 'build'], {
    stdout: 'inherit',
    stderr: 'inherit',
    env: envVars,
  });

  if (!tauriBuild.success) {
    console.error('❌ Tauri build failed');
    process.exit(1);
  }
} else {
  console.log(`⏩ [1/4 & 2/4] Skipped build step (--skip-build). Using existing bundle.`);
}

// 3. Scan & Sign Artifacts
console.log(`\n🔍 [3/4] Scanning release artifacts & signatures...`);
const bundleDir = resolve('src-tauri/target/release/bundle');
const platforms: Record<string, { signature: string; url: string }> = {};
const filesToUpload: Array<{ path: string; key: string; contentType: string }> = [];

function scanForArtifacts(dir: string, platformKey: string) {
  if (!existsSync(dir)) return;
  const files = readdirSync(dir);

  for (const file of files) {
    if (file.endsWith('.sig')) {
      const targetFile = file.slice(0, -4);
      const targetPath = join(dir, targetFile);
      const sigPath = join(dir, file);

      if (existsSync(targetPath)) {
        const signature = readFileSync(sigPath, 'utf8').trim();
        const downloadUrl = `${publicUrl.replace(/\/$/, '')}/v${version}/${basename(targetFile)}`;
        platforms[platformKey] = {
          signature,
          url: downloadUrl,
        };
        const isExe = targetFile.endsWith('.exe');
        const isMsi = targetFile.endsWith('.msi');
        const isZip = targetFile.endsWith('.zip');
        const contentType = isExe
          ? 'application/vnd.microsoft.portable-executable'
          : isMsi
          ? 'application/x-msi'
          : isZip
          ? 'application/zip'
          : 'application/octet-stream';

        filesToUpload.push({
          path: targetPath,
          key: `v${version}/${basename(targetFile)}`,
          contentType,
        });
        filesToUpload.push({
          path: sigPath,
          key: `v${version}/${file}`,
          contentType: 'text/plain',
        });
        const sizeMb = (statSync(targetPath).size / 1024 / 1024).toFixed(2);
        console.log(`   ✅ ${platformKey}: ${targetFile} (${sizeMb} MB) [Signed]`);
      }
    } else if (file.endsWith('.deb') || file.endsWith('.rpm') || file.endsWith('.AppImage')) {
      const targetPath = join(dir, file);
      if (!statSync(targetPath).isDirectory()) {
        filesToUpload.push({
          path: targetPath,
          key: `v${version}/${file}`,
          contentType: file.endsWith('.deb') ? 'application/vnd.debian.binary-package' : file.endsWith('.rpm') ? 'application/x-rpm' : 'application/octet-stream',
        });
        const sizeMb = (statSync(targetPath).size / 1024 / 1024).toFixed(2);
        console.log(`   📦 Direct Installer (${platformKey}): ${file} (${sizeMb} MB)`);
      }
    }
  }
}

// Check all possible bundle directories (including cross-compilation target triples)
const targetDirs = [
  { dir: resolve('src-tauri/target/release/bundle'), macKey: 'darwin-x86_64' },
  { dir: resolve('src-tauri/target/x86_64-pc-windows-msvc/release/bundle'), macKey: 'darwin-x86_64' },
  { dir: resolve('src-tauri/target/x86_64-unknown-linux-gnu/release/bundle'), macKey: 'darwin-x86_64' },
  { dir: resolve('src-tauri/target/aarch64-apple-darwin/release/bundle'), macKey: 'darwin-aarch64' },
  { dir: resolve('src-tauri/target/x86_64-apple-darwin/release/bundle'), macKey: 'darwin-x86_64' },
];

for (const { dir: bDir, macKey } of targetDirs) {
  // Windows
  scanForArtifacts(join(bDir, 'nsis'), 'windows-x86_64');
  scanForArtifacts(join(bDir, 'msi'), 'windows-x86_64');

  // Linux
  scanForArtifacts(join(bDir, 'appimage'), 'linux-x86_64');
  scanForArtifacts(join(bDir, 'deb'), 'linux-x86_64');

  // macOS
  scanForArtifacts(join(bDir, 'dmg'), macKey);
  scanForArtifacts(join(bDir, 'macos'), macKey);
}

// Fetch and merge existing platforms from R2 if same version
let mergedPlatforms = { ...platforms };
try {
  const existingRes = await fetch(`${publicUrl.replace(/\/$/, '')}/latest.json`, { cache: 'no-store' });
  if (existingRes.ok) {
    const existingJson = await existingRes.json();
    if (existingJson.version === version && existingJson.platforms) {
      mergedPlatforms = {
        ...existingJson.platforms,
        ...platforms,
      };
      console.log(`🔄 Merged platforms for v${version}:`, Object.keys(mergedPlatforms));
    }
  }
} catch {
  // Ignore network error on first release
}

// Generate latest.json
const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: mergedPlatforms,
};

const outputManifestPath = resolve('latest.json');
writeFileSync(outputManifestPath, JSON.stringify(manifest, null, 2), 'utf8');
console.log(`📄 Generated latest.json saved to: ${outputManifestPath}`);

// 4. Upload to Cloudflare R2
if (dryRun) {
  console.log(`\n🛑 [4/4] Dry-run enabled. Skipped uploading to Cloudflare R2.`);
  process.exit(0);
}

if (!accessKeyId || !secretAccessKey) {
  console.error('\n❌ Missing R2_ACCESS_KEY_ID or R2_SECRET_ACCESS_KEY in .env. Cannot upload to Cloudflare R2.');
  process.exit(1);
}

console.log(`\n☁️ [4/4] Uploading artifacts & latest.json to Cloudflare R2 (${bucketName})...`);
const s3 = new S3Client({
  region: 'auto',
  endpoint,
  credentials: {
    accessKeyId,
    secretAccessKey,
  },
});

for (const item of filesToUpload) {
  const body = readFileSync(item.path);
  const sizeMb = (body.length / 1024 / 1024).toFixed(2);
  console.log(`   📤 Uploading ${item.key} (${sizeMb} MB)...`);
  await s3.send(
    new PutObjectCommand({
      Bucket: bucketName,
      Key: item.key,
      Body: body,
      ContentType: item.contentType,
    })
  );
  console.log(`   ✅ Succeeded: ${item.key}`);
}

// Upload latest.json with no-cache header
const manifestBody = readFileSync(outputManifestPath);
console.log(`   📤 Uploading latest.json (root)...`);
await s3.send(
  new PutObjectCommand({
    Bucket: bucketName,
    Key: 'latest.json',
    Body: manifestBody,
    ContentType: 'application/json',
    CacheControl: 'no-cache, no-store, must-revalidate',
  })
);
console.log(`   ✅ Succeeded: latest.json`);

// 5. Verification
console.log(`\n======================================================`);
console.log(`🎉 PHÁT HÀNH BẢN V${version} LÊN CLOUDFLARE R2 THÀNH CÔNG!`);
console.log(`======================================================`);
console.log(`🔗 Manifest URL: ${publicUrl.replace(/\/$/, '')}/latest.json`);
for (const [platform, entry] of Object.entries(platforms)) {
  console.log(`📥 ${platform} binary: ${entry.url}`);
}
console.log(`======================================================\n`);
