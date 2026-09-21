// Keep production app dependencies separated; test-only peers are intentional.
import { spawnSync } from 'node:child_process';
const rules = {
  'peakrunner-launcher': ['peakrunner-core', 'peakrunner-protocol', 'peakrunner-server', 'peakrunner-net', 'peakrunner-directory', 'rodio'],
  'peakrunner-directory': ['peakrunner-core', 'peakrunner-protocol', 'peakrunner-net', 'peakrunner-server', 'glam', 'eframe', 'wgpu', 'rodio', 'postcard', 'lz4_flex'],
  'peakrunner-server': ['peakrunner-net', 'peakrunner-directory', 'eframe', 'wgpu', 'rodio'],
  'peakrunner': ['peakrunner-server', 'peakrunner-directory'],
};
for (const [app, forbidden] of Object.entries(rules)) {
  const result = spawnSync('cargo', ['tree', '--locked', '-p', app, '--edges', 'normal', '--prefix', 'none'], { encoding: 'utf8' });
  if (result.status !== 0) throw new Error(result.stderr || 'cargo tree failed');
  const packages = new Set(result.stdout.split('\n').map(line => line.split(' ')[0]));
  for (const dependency of forbidden) {
    if (packages.has(dependency)) throw new Error(`${app} must not depend on ${dependency}`);
  }
  console.log(`PASS: ${app} production dependency boundary`);
}
