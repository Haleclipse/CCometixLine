const fs = require('fs');
const path = require('path');
const os = require('os');

// Silent mode detection
const silent = process.env.npm_config_loglevel === 'silent' || 
               process.env.CCLINE_SKIP_POSTINSTALL === '1';

if (!silent) {
  console.log('🚀 Setting up CCometixLine for Claude Code...');
}

try {
  const platform = process.platform;
  const arch = process.arch;
  const homeDir = os.homedir();
  const claudeDir = path.join(homeDir, '.claude', 'ccline');

  // Create directory
  fs.mkdirSync(claudeDir, { recursive: true });

  // Determine platform key
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
    'win32-ia32': '@cometix/ccline-win32-x64', // Use 64-bit for 32-bit
  };

  const packageName = packageMap[platformKey];
  if (!packageName) {
    if (!silent) {
      console.log(`Platform ${platformKey} not supported for auto-setup`);
    }
    process.exit(0);
  }

  const binaryName = platform === 'win32' ? 'ccline.exe' : 'ccline';
  const targetPath = path.join(claudeDir, binaryName);

  // Multiple path search strategies for different package managers
  const findBinaryPath = () => {
    const possiblePaths = [
      // npm/yarn: nested in node_modules
      path.join(__dirname, '..', 'node_modules', packageName, binaryName),
      // pnpm: try require.resolve first
      (() => {
        try {
          const packagePath = require.resolve(packageName + '/package.json');
          return path.join(path.dirname(packagePath), binaryName);
        } catch {
          return null;
        }
      })(),
      // pnpm: flat structure fallback with version detection
      (() => {
        const currentPath = __dirname;
        const pnpmMatch = currentPath.match(/(.+\.pnpm)[\\/]([^\\//]+)[\\/]/);
        if (pnpmMatch) {
          const pnpmRoot = pnpmMatch[1];
          const packageNameEncoded = packageName.replace('/', '+');
          
          try {
            // Try to find any version of the package
            const pnpmContents = fs.readdirSync(pnpmRoot);
            const packagePattern = new RegExp(`^${packageNameEncoded.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}@`);
            const matchingPackage = pnpmContents.find(dir => packagePattern.test(dir));
            
            if (matchingPackage) {
              return path.join(pnpmRoot, matchingPackage, 'node_modules', packageName, binaryName);
            }
          } catch {
            // Fallback to current behavior if directory reading fails
          }
        }
        return null;
      })()
    ].filter(p => p !== null);

    for (const testPath of possiblePaths) {
      if (fs.existsSync(testPath)) {
        return testPath;
      }
    }
    return null;
  };

  const sourcePath = findBinaryPath();
  if (!sourcePath) {
    if (!silent) {
      console.log('Binary package not installed, skipping Claude Code setup');
      console.log('The global ccline command will still work via npm');
    }
    process.exit(0);
  }

  // Copy or link the binary
  if (platform === 'win32') {
    // Windows: Copy file
    fs.copyFileSync(sourcePath, targetPath);
  } else {
    // Unix: Try hard link first, fallback to copy
    try {
      if (fs.existsSync(targetPath)) {
        fs.unlinkSync(targetPath);
      }
      fs.linkSync(sourcePath, targetPath);
    } catch {
      fs.copyFileSync(sourcePath, targetPath);
    }
    fs.chmodSync(targetPath, '755');
  }

  if (!silent) {
    console.log('✨ CCometixLine is ready for Claude Code!');
    console.log(`📍 Location: ${targetPath}`);
    console.log('🎉 You can now use: ccline --help');
  }
} catch (error) {
  // Silent failure - don't break installation
  if (!silent) {
    console.log('Note: Could not auto-configure for Claude Code');
    console.log('The global ccline command will still work.');
    console.log('You can manually copy ccline to ~/.claude/ccline/ if needed');
  }
}