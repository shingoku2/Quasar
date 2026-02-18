#!/usr/bin/env node
/**
 * Run Tauri dev with CARGO_TARGET_DIR set to this project's absolute path.
 * Overrides any global CARGO_TARGET_DIR (e.g. from a renamed/moved project).
 */
import path from 'path';
import { spawn, execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '..');
const targetDir = path.join(projectRoot, 'src-tauri', 'target');

function ensureNpxAvailable() {
  try {
    if (process.platform === 'win32') {
      execSync('where npx', { stdio: 'ignore' });
    } else {
      execSync('command -v npx', { stdio: 'ignore' });
    }
  } catch {
    console.error('npx was not found. Ensure Node.js and npm are installed and npx is in your PATH.');
    process.exit(1);
  }
}

const env = { ...process.env, CARGO_TARGET_DIR: targetDir };
const args = ['tauri', ...process.argv.slice(2)];
// On Windows, npx is typically npx.cmd and needs shell to run; without shell we get spawn EINVAL.
const useShell = process.platform === 'win32';

ensureNpxAvailable();

const child = spawn('npx', args, {
  stdio: 'inherit',
  env,
  cwd: projectRoot,
  shell: useShell,
});

child.on('close', (code) => process.exit(code ?? 1));
child.on('error', (err) => {
  console.error('Failed to start npx:', err.message);
  console.error('Ensure Node.js and npm are installed and npx is in your PATH.');
  process.exit(1);
});
