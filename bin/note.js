#!/usr/bin/env node
'use strict';

// Thin launcher: runs the native `note` binary from ../vendor.

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const installer = require('../scripts/install.js');

const binary = installer.binaryPath();

function run() {
  const result = spawnSync(binary, process.argv.slice(2), { stdio: 'inherit' });
  if (result.error) {
    console.error(`note: could not run ${binary}: ${result.error.message}`);
    return 1;
  }
  if (result.signal) {
    process.kill(process.pid, result.signal);
    return 1;
  }
  return result.status === null ? 1 : result.status;
}

if (fs.existsSync(binary)) {
  process.exitCode = run();
} else if (!installer.targetFor(process.platform, process.arch)) {
  console.error(installer.unsupportedMessage(process.platform, process.arch));
  process.exitCode = 1;
} else {
  // Binary missing (install script skipped or failed): try once more now.
  const installScript = path.join(__dirname, '..', 'scripts', 'install.js');
  spawnSync(process.execPath, [installScript], { stdio: ['ignore', 'ignore', 'inherit'] });
  if (fs.existsSync(binary)) {
    process.exitCode = run();
  } else {
    console.error('note: the native binary is missing; see the messages above.');
    process.exitCode = 1;
  }
}
