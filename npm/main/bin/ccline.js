#!/usr/bin/env node
const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

// 1. Priority: Use ~/.claude/ccline/ccline if exists
const claudePath = path.join(
  os.homedir(), 
  '.claude', 
  'ccline',
  process.platform === 'win32' ? 'ccline.exe' : 'ccline'
);

if (fs.existsSync(claudePath)) {
  const result = spawnSync(claudePath, process.argv.slice(2), {
    stdio: 'inherit',
    shell: false
  });
  process.exit(result.status || 0);
}

// 2. Fallback: Use npm package binary
const platform = process.platform;
const arch = process.arch;

// Handle special cases
let platformKey = `${platform}-${arch}`;
if (platform === 'linux') {
  // Detect libc type and version
  function getLibcInfo() {
    try {
      const { execSync } = require('child_process');
      const lddOutput = execSync('ldd --version 2>/dev/null || echo ""', {
        encoding: 'utf8',
        timeout: 1000
      });

      // Check for musl explicitly
      if (lddOutput.includes('musl')) {
        return { type: 'musl' };
      }

      // Parse glibc version: "ldd (GNU libc) 2.35" format
      const match = lddOutput.match(/(?:GNU libc|GLIBC).*?(\d+)\.(\d+)/);
      if (match) {
        const major = parseInt(match[1]);
        const minor = parseInt(match[2]);
        return { type: 'glibc', major, minor };
      }

      // If we can't detect, default to musl for safety (more portable)
      return { type: 'musl' };
    } catch (e) {
      // If detection fails, default to musl (more portable)
      return { type: 'musl' };
    }
  }

  const libcInfo = getLibcInfo();

  if (arch === 'arm64') {
    // ARM64 Linux: choose based on libc type and version
    if (libcInfo.type === 'musl' ||
        (libcInfo.type === 'glibc' && (libcInfo.major < 2 || (libcInfo.major === 2 && libcInfo.minor < 35)))) {
      platformKey = 'linux-arm64-musl';
    } else {
      platformKey = 'linux-arm64';
    }
  } else {
    // x64 Linux: choose based on libc type and version
    if (libcInfo.type === 'musl' ||
        (libcInfo.type === 'glibc' && (libcInfo.major < 2 || (libcInfo.major === 2 && libcInfo.minor < 35)))) {
      platformKey = 'linux-x64-musl';
    }
  }
}

const packageMap = {
  'darwin-x64': '@cometix/ccline-darwin-x64',
  'darwin-arm64': '@cometix/ccline-darwin-arm64',
  'linux-x64': '@cometix/ccline-linux-x64',
  'linux-x64-musl': '@cometix/ccline-linux-x64-musl',
  'linux-arm64': '@cometix/ccline-linux-arm64',
  'linux-arm64-musl': '@cometix/ccline-linux-arm64-musl',
  'win32-x64': '@cometix/ccline-win32-x64',
  'win32-ia32': '@cometix/ccline-win32-x64', // Use 64-bit for 32-bit systems
};

const packageName = packageMap[platformKey];
if (!packageName) {
  console.error(`Error: Unsupported platform ${platformKey}`);
  console.error('Supported platforms: darwin (x64/arm64), linux (x64/arm64), win32 (x64)');
  console.error('Please visit https://github.com/Haleclipse/CCometixLine for manual installation');
  process.exit(1);
}

const binaryName = platform === 'win32' ? 'ccline.exe' : 'ccline';
const binaryPath = path.join(__dirname, '..', 'node_modules', packageName, binaryName);

if (!fs.existsSync(binaryPath)) {
  console.error(`Error: Binary not found at ${binaryPath}`);
  console.error('This might indicate a failed installation or unsupported platform.');
  console.error('Please try reinstalling: npm install -g @cometix/ccline');
  console.error(`Expected package: ${packageName}`);
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: 'inherit',
  shell: false
});

process.exit(result.status || 0);