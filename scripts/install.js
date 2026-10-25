#!/usr/bin/env node
'use strict';

// Fetches the prebuilt `note` binary for this machine into ../vendor.
// Node built-ins only. Runs as postinstall, and again from bin/note.js if the
// binary is missing (for example after `npm install --ignore-scripts`).

const crypto = require('crypto');
const fs = require('fs');
const https = require('https');
const path = require('path');
const zlib = require('zlib');
const { spawnSync } = require('child_process');

const pkg = require('../package.json');

const REPO = 'baiehclaca/note';

// process.platform-process.arch -> Rust target triple of the release asset.
// To add a platform: build that target in .github/workflows/release.yml,
// add it here, and add its os/cpu to package.json.
const TARGETS = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
};

const VENDOR_DIR = path.join(__dirname, '..', 'vendor');

function binaryName(platform) {
  return platform === 'win32' ? 'note.exe' : 'note';
}

function binaryPath(platform = process.platform) {
  return path.join(VENDOR_DIR, binaryName(platform));
}

function targetFor(platform, arch) {
  return TARGETS[`${platform}-${arch}`] || null;
}

function unsupportedMessage(platform, arch) {
  return `note: no prebuilt binary for ${platform}-${arch}. note is macOS only for now; Windows and Linux are coming.`;
}

function assetName(version, target) {
  return `note-${version}-${target}.tar.gz`;
}

function assetUrl(version, target) {
  return `https://github.com/${REPO}/releases/download/v${version}/${assetName(version, target)}`;
}

// Accepts `shasum -a 256` / `sha256sum` output ("<hex>  <file>") or a bare hash.
function parseChecksum(text) {
  const match = /^\s*([0-9a-fA-F]{64})(?:\s|$)/.exec(String(text));
  if (!match) throw new Error('checksum file is not a SHA-256 hash');
  return match[1].toLowerCase();
}

function sha256(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

function verifyChecksum(buffer, checksumText) {
  const expected = parseChecksum(checksumText);
  const actual = sha256(buffer);
  if (actual !== expected) {
    throw new Error(`checksum mismatch: expected ${expected}, got ${actual}`);
  }
}

// Minimal tar reader: returns the contents of the regular file whose base
// name is `name`.
function extractFromTar(tar, name) {
  let offset = 0;
  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) break;
    const field = (start, length) =>
      header.subarray(start, start + length).toString('utf8').replace(/\0.*$/s, '');
    const prefix = field(345, 155);
    const entry = prefix ? `${prefix}/${field(0, 100)}` : field(0, 100);
    const size = parseInt(field(124, 12).trim() || '0', 8);
    const type = field(156, 1);
    if (Number.isNaN(size)) throw new Error('corrupt tar header');
    const start = offset + 512;
    if ((type === '0' || type === '') && path.posix.basename(entry) === name) {
      if (start + size > tar.length) throw new Error('truncated tar archive');
      return tar.subarray(start, start + size);
    }
    offset = start + Math.ceil(size / 512) * 512;
  }
  throw new Error(`${name} not found in archive`);
}

function download(url, redirects = 5) {
  return new Promise((resolve, reject) => {
    const request = https.get(url, { headers: { 'User-Agent': `${pkg.name}/${pkg.version}` } }, (res) => {
      const { statusCode, headers } = res;
      if (statusCode >= 300 && statusCode < 400 && headers.location) {
        res.resume();
        if (redirects <= 0) return reject(new Error(`too many redirects for ${url}`));
        return resolve(download(new URL(headers.location, url).toString(), redirects - 1));
      }
      if (statusCode !== 200) {
        res.resume();
        return reject(new Error(`HTTP ${statusCode} for ${url}`));
      }
      const chunks = [];
      res.on('data', (chunk) => chunks.push(chunk));
      res.on('end', () => resolve(Buffer.concat(chunks)));
      res.on('error', reject);
    });
    request.setTimeout(60000, () => request.destroy(new Error(`timed out downloading ${url}`)));
    request.on('error', reject);
  });
}

function writeBinary(contents, platform) {
  fs.mkdirSync(VENDOR_DIR, { recursive: true });
  const dest = binaryPath(platform);
  const tmp = `${dest}.tmp-${process.pid}`;
  fs.writeFileSync(tmp, contents);
  fs.chmodSync(tmp, 0o755);
  fs.renameSync(tmp, dest);
  return dest;
}

async function installPrebuilt(version, target, platform) {
  const url = assetUrl(version, target);
  const [archive, checksum] = await Promise.all([download(url), download(`${url}.sha256`)]);
  verifyChecksum(archive, checksum.toString('utf8'));
  const binary = extractFromTar(zlib.gunzipSync(archive), binaryName(platform));
  return writeBinary(binary, platform);
}

function hasCargo() {
  const result = spawnSync('cargo', ['--version'], { stdio: 'ignore' });
  return result.status === 0;
}

function installWithCargo(version, platform) {
  const root = path.join(VENDOR_DIR, 'cargo');
  const args = ['install', '--git', `https://github.com/${REPO}`, '--tag', `v${version}`, '--locked', '--root', root, 'note'];
  console.error(`note: building from source with cargo ${args.join(' ')}`);
  const result = spawnSync('cargo', args, { stdio: 'inherit' });
  if (result.status !== 0) throw new Error('cargo install failed');
  const built = path.join(root, 'bin', binaryName(platform));
  const dest = writeBinary(fs.readFileSync(built), platform);
  fs.rmSync(root, { recursive: true, force: true });
  return dest;
}

// Returns the installed binary path, or null when this platform has no build.
// Never throws: install must not fail; bin/note.js reports problems on use.
async function install({ platform = process.platform, arch = process.arch, version = pkg.version } = {}) {
  const target = targetFor(platform, arch);
  if (!target) {
    console.error(unsupportedMessage(platform, arch));
    return null;
  }
  try {
    return await installPrebuilt(version, target, platform);
  } catch (error) {
    console.error(`note: could not download the prebuilt binary (${error.message}).`);
  }
  if (hasCargo()) {
    try {
      return installWithCargo(version, platform);
    } catch (error) {
      console.error(`note: ${error.message}.`);
    }
  }
  console.error(
    `note: install incomplete. Check your connection and run \`npm install -g ${pkg.name}\` again,\n` +
      `or install Rust (https://rustup.rs) and run \`cargo install --git https://github.com/${REPO}\`.`
  );
  return null;
}

module.exports = {
  TARGETS,
  assetName,
  assetUrl,
  binaryName,
  binaryPath,
  extractFromTar,
  install,
  parseChecksum,
  sha256,
  targetFor,
  unsupportedMessage,
  verifyChecksum,
};

if (require.main === module) {
  install().then(() => process.exit(0));
}
