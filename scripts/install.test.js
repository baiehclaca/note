'use strict';

// Run with: npm test (node --test). Not shipped in the npm package.

const assert = require('assert');
const fs = require('fs');
const os = require('os');
const path = require('path');
const test = require('node:test');
const zlib = require('zlib');
const { spawnSync } = require('child_process');

const installer = require('./install.js');
const pkg = require('../package.json');

const root = path.join(__dirname, '..');

test('maps supported platforms to release targets', () => {
  assert.strictEqual(installer.targetFor('darwin', 'arm64'), 'aarch64-apple-darwin');
  assert.strictEqual(installer.targetFor('darwin', 'x64'), 'x86_64-apple-darwin');
  assert.strictEqual(installer.targetFor('linux', 'x64'), 'x86_64-unknown-linux-gnu');
  assert.strictEqual(installer.targetFor('linux', 'arm64'), 'aarch64-unknown-linux-gnu');
  assert.strictEqual(installer.targetFor('win32', 'x64'), 'x86_64-pc-windows-msvc');
  assert.strictEqual(installer.targetFor('win32', 'arm64'), null);
  assert.strictEqual(installer.targetFor('darwin', 'ia32'), null);
});

test('package.json os/cpu cover every target in the table', () => {
  for (const key of Object.keys(installer.TARGETS)) {
    const [platform, arch] = key.split('-');
    assert.ok(pkg.os.includes(platform), `${platform} missing from package.json os`);
    assert.ok(pkg.cpu.includes(arch), `${arch} missing from package.json cpu`);
  }
});

test('version matches Cargo.toml', () => {
  const cargo = fs.readFileSync(path.join(root, 'Cargo.toml'), 'utf8');
  const version = /^version\s*=\s*"([^"]+)"/m.exec(cargo)[1];
  assert.strictEqual(pkg.version, version);
});

test('builds release asset names and URLs', () => {
  assert.strictEqual(installer.assetName('1.2.3', 'aarch64-apple-darwin'), 'note-1.2.3-aarch64-apple-darwin.tar.gz');
  assert.strictEqual(
    installer.assetUrl('1.2.3', 'x86_64-apple-darwin'),
    'https://github.com/baiehclaca/note/releases/download/v1.2.3/note-1.2.3-x86_64-apple-darwin.tar.gz'
  );
});

test('binary name per platform', () => {
  assert.strictEqual(installer.binaryName('darwin'), 'note');
  assert.strictEqual(installer.binaryName('win32'), 'note.exe');
});

test('parses shasum output and bare hashes', () => {
  const hex = 'a'.repeat(64);
  assert.strictEqual(installer.parseChecksum(`${hex}  note-1.0.0-aarch64-apple-darwin.tar.gz\n`), hex);
  assert.strictEqual(installer.parseChecksum(`${'A'.repeat(64)}\n`), hex);
  assert.throws(() => installer.parseChecksum('not a hash'));
  assert.throws(() => installer.parseChecksum('a'.repeat(63)));
  assert.throws(() => installer.parseChecksum(''));
});

test('verifies checksums', () => {
  const data = Buffer.from('hello');
  const good = '2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824';
  assert.strictEqual(installer.sha256(data), good);
  assert.doesNotThrow(() => installer.verifyChecksum(data, `${good}  file`));
  assert.throws(() => installer.verifyChecksum(Buffer.from('tampered'), `${good}  file`), /checksum mismatch/);
});

test('extracts the binary from a tar.gz made by system tar', { skip: !hasTar() }, () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'note-tar-'));
  try {
    fs.writeFileSync(path.join(dir, 'README'), 'x'.repeat(700));
    fs.writeFileSync(path.join(dir, 'note'), 'binary-bytes');
    const archive = path.join(dir, 'a.tar.gz');
    const made = spawnSync('tar', ['-C', dir, '-czf', archive, 'README', 'note']);
    assert.strictEqual(made.status, 0);
    const tar = zlib.gunzipSync(fs.readFileSync(archive));
    assert.strictEqual(installer.extractFromTar(tar, 'note').toString(), 'binary-bytes');
    assert.throws(() => installer.extractFromTar(tar, 'note.exe'), /not found/);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test('unsupported platform: install resolves to null without throwing', async () => {
  const result = await installer.install({ platform: 'freebsd', arch: 'x64' });
  assert.strictEqual(result, null);
});

test('launcher runs the vendored binary, passes args and exit code', { skip: process.platform === 'win32' }, () => {
  // Copy the package to a temp dir so the real vendor/ is never touched.
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'note-pkg-'));
  try {
    for (const file of ['package.json', 'bin/note.js', 'scripts/install.js']) {
      fs.mkdirSync(path.dirname(path.join(dir, file)), { recursive: true });
      fs.copyFileSync(path.join(root, file), path.join(dir, file));
    }
    fs.mkdirSync(path.join(dir, 'vendor'));
    const fake = path.join(dir, 'vendor', 'note');
    fs.writeFileSync(fake, '#!/bin/sh\necho "fake note: $*"\nexit 7\n');
    fs.chmodSync(fake, 0o755);
    const result = spawnSync(process.execPath, [path.join(dir, 'bin', 'note.js'), 'find', 'prague pitch'], {
      encoding: 'utf8',
    });
    assert.strictEqual(result.stdout, 'fake note: find prague pitch\n');
    assert.strictEqual(result.status, 7);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

function hasTar() {
  return spawnSync('tar', ['--version'], { stdio: 'ignore' }).status === 0;
}
