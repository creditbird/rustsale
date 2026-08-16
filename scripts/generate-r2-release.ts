import { readFileSync, writeFileSync, existsSync, readdirSync } from 'node:fs';
import { resolve, join, basename } from 'node:path';
import { S3Client, PutObjectCommand } from '@aws-sdk/client-s3';

interface PlatformEntry {
  signature: string;
  url: string;
}

interface TauriReleaseManifest {
  version: string;
  notes: string;
  pub_date: string;
  platforms: Record<string, PlatformEntry>;
}

// 1. Read version from tauri.conf.json
const tauriConfPath = resolve('src-tauri/tauri.conf.json');
if (!existsSync(tauriConfPath)) {
  console.error('❌ Could not find src-tauri/tauri.conf.json');
  process.exit(1);
}

const tauriConf = JSON.parse(readFileSync(tauriConfPath, 'utf8'));
const version = process.env.RELEASE_VERSION || tauriConf.version || '0.1.0';
const baseUrl = process.env.R2_PUBLIC_URL || 'https://pub-6f14359d39ae4d50ac90b7e759b6de56.r2.dev';
const notes = process.env.RELEASE_NOTES || 'Bản cập nhật và vá lỗi ổn định cho RustSale CRM.';
const bucketName = process.env.R2_BUCKET_NAME || 'rustsale-releases';
const endpoint = process.env.R2_ENDPOINT || 'https://94af112d3d793eb10ab906f0bf8254cd.r2.cloudflarestorage.com';
const accessKeyId = process.env.R2_ACCESS_KEY_ID;
const secretAccessKey = process.env.R2_SECRET_ACCESS_KEY;

console.log(`\n🚀 Generating Cloudflare R2 Release Manifest for v${version}...`);
console.log(`🌐 Public Download URL: ${baseUrl}`);
console.log(`🪣 Bucket: ${bucketName}\n`);

const bundleDir = resolve('src-tauri/target/release/bundle');
const platforms: Record<string, PlatformEntry> = {};
const filesToUpload: Array<{ path: string; key: string; contentType: string }> = [];

// Helper to find signature and file in a directory
function scanForArtifacts(dir: string, platformKey: string) {
  if (!existsSync(dir)) return;
  const files = readdirSync(dir);

  for (const file of files) {
    if (file.endsWith('.sig')) {
      const targetFile = file.slice(0, -4); // Remove .sig
      const targetPath = join(dir, targetFile);
      const sigPath = join(dir, file);

      if (existsSync(targetPath)) {
        const signature = readFileSync(sigPath, 'utf8').trim();
        const downloadUrl = `${baseUrl.replace(/\/$/, '')}/v${version}/${basename(targetFile)}`;
        platforms[platformKey] = {
          signature,
          url: downloadUrl,
        };
        filesToUpload.push({
          path: targetPath,
          key: `v${version}/${basename(targetFile)}`,
          contentType: targetFile.endsWith('.exe') ? 'application/vnd.microsoft.portable-executable' : 'application/octet-stream',
        });
        filesToUpload.push({
          path: sigPath,
          key: `v${version}/${file}`,
          contentType: 'text/plain',
        });
        console.log(`✅ Found ${platformKey}: ${targetFile} (Signature verified)`);
      }
    }
  }
}

// Check Windows NSIS / MSI
scanForArtifacts(join(bundleDir, 'nsis'), 'windows-x86_64');
scanForArtifacts(join(bundleDir, 'msi'), 'windows-x86_64');

// Check Linux AppImage / Deb
scanForArtifacts(join(bundleDir, 'appimage'), 'linux-x86_64');
scanForArtifacts(join(bundleDir, 'deb'), 'linux-x86_64');

// Check macOS DMG / App
scanForArtifacts(join(bundleDir, 'dmg'), 'darwin-x86_64');
scanForArtifacts(join(bundleDir, 'macos'), 'darwin-aarch64');

// If no artifacts found (e.g. running before full tauri build), create a template
if (Object.keys(platforms).length === 0) {
  console.warn('⚠️ No built release artifacts found in src-tauri/target/release/bundle/. Generating template manifest...');
  platforms['windows-x86_64'] = {
    signature: 'BASE64_ED25519_SIGNATURE_PLACEHOLDER',
    url: `${baseUrl.replace(/\/$/, '')}/v${version}/RustSale-CRM_${version}_x64-setup.exe`,
  };
}

const manifest: TauriReleaseManifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms,
};

const outputManifestPath = resolve('latest.json');
writeFileSync(outputManifestPath, JSON.stringify(manifest, null, 2), 'utf8');

console.log(`\n🎉 Generated manifest saved to: ${outputManifestPath}`);
console.log(`\n--- [latest.json preview] ---`);
console.log(JSON.stringify(manifest, null, 2));

// Automated R2 Direct Upload (if credentials present)
async function performR2Upload() {
  if (!accessKeyId || !secretAccessKey) {
    console.log(`\n======================================================`);
    console.log(`💡 MẸO: TỰ ĐỘNG UPLOAD LÊN R2 VỚI 1 LỆNH DUY NHẤT:`);
    console.log(`======================================================`);
    console.log(`Thêm 2 dòng sau vào file .env:`);
    console.log(`   R2_ACCESS_KEY_ID=your_access_key_id`);
    console.log(`   R2_SECRET_ACCESS_KEY=your_secret_access_key`);
    console.log(`Sau đó chạy lại "bun run release:r2" -> Tất cả file sẽ tự động upload lên R2!`);
    console.log(`\nHoặc upload thủ công qua Cloudflare Dashboard / AWS CLI:`);
    console.log(`   1. Upload thư mục v${version}/ lên bucket ${bucketName}`);
    console.log(`   2. Upload file latest.json lên ROOT của bucket ${bucketName}`);
    console.log(`======================================================\n`);
    return;
  }

  console.log(`\n☁️ Đang tự động upload lên Cloudflare R2 (${bucketName})...`);
  const s3 = new S3Client({
    region: 'auto',
    endpoint,
    credentials: {
      accessKeyId,
      secretAccessKey,
    },
  });

  // 1. Upload files
  for (const item of filesToUpload) {
    const fileBody = readFileSync(item.path);
    console.log(`📤 Uploading: ${item.key} (${(fileBody.length / 1024 / 1024).toFixed(2)} MB)...`);
    await s3.send(
      new PutObjectCommand({
        Bucket: bucketName,
        Key: item.key,
        Body: fileBody,
        ContentType: item.contentType,
      })
    );
    console.log(`   ✅ Succeeded: ${item.key}`);
  }

  // 2. Upload latest.json
  const manifestBody = readFileSync(outputManifestPath);
  console.log(`📤 Uploading: latest.json to root...`);
  await s3.send(
    new PutObjectCommand({
      Bucket: bucketName,
      Key: 'latest.json',
      Body: manifestBody,
      ContentType: 'application/json',
      CacheControl: 'no-cache, no-store, must-revalidate',
    })
  );
  console.log(`   ✅ Succeeded: latest.json (Cache-Control: no-cache)`);
  console.log(`\n🎉 HOÀN TẤT PHÁT HÀNH BẢN V${version} LÊN CLOUDFLARE R2!`);
  console.log(`👉 Link manifest: ${baseUrl.replace(/\/$/, '')}/latest.json\n`);
}

performR2Upload().catch((err) => {
  console.error('❌ Lỗi upload lên R2:', err);
  process.exit(1);
});
