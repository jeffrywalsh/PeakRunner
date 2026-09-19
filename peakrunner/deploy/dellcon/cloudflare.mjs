// Run on an administrator machine with Node 22+. No credentials are printed.
import fs from 'node:fs';
import {spawnSync} from 'node:child_process';

const spec = JSON.parse(fs.readFileSync(new URL('./cloudflare.json', import.meta.url)));
const args = new Set(process.argv.slice(2));
const apply = args.has('--apply');
const start = args.has('--start');
const sync = args.has('--sync-config');
const host = process.env.PEAKRUNNER_SSH_HOST || 'jeffryw@192.168.1.64';
const directory = process.env.PEAKRUNNER_DEPLOY_DIR || '/data/peakrunner/public';
const fail = message => { throw new Error(message); };
const quote = s => "'" + s.replaceAll("'", "'\\''") + "'";

async function main() {
  if ([...args].some(a => !['--apply', '--start', '--sync-config'].includes(a))) fail('Unknown argument.');
  if (sync && !apply) fail('--sync-config requires --apply.');
  if (!process.env.CF_API_TOKEN_FILE) fail('Set CF_API_TOKEN_FILE to your protected token file.');
  if (!/^[A-Za-z0-9_.@:-]+$/.test(host) || host.startsWith('-')) fail('Invalid SSH destination.');
  const source = fs.readFileSync(process.env.CF_API_TOKEN_FILE, 'utf8').trim();
  const token = source.match(/Bearer\s+([A-Za-z0-9_-]+)/i)?.[1]
    ?? (/^[A-Za-z0-9_-]+$/.test(source) ? source : undefined);
  if (!token) fail('Unrecognized token file; contents withheld.');
  async function api(path, method = 'GET', body) {
    const response = await fetch('https://api.cloudflare.com/client/v4' + path, {
      method, headers: {Authorization: 'Bearer ' + token, 'Content-Type': 'application/json'},
      body: body ? JSON.stringify(body) : undefined, signal: AbortSignal.timeout(20000),
    });
    const data = await response.json();
    if (!response.ok || !data.success) fail(`Cloudflare ${method}: HTTP ${response.status}; codes ${(data.errors ?? []).map(e => e.code).join(',')}`);
    return data.result;
  }
  const zones = await api('/zones?name=' + encodeURIComponent(spec.zone));
  if (zones.length !== 1 || zones[0].status !== 'active') fail('Expected one active zone.');
  const zone = zones[0];
  const base = `/accounts/${zone.account.id}/cfd_tunnel`;
  const tunnels = (await api(base + '?is_deleted=false')).filter(t => t.name === spec.tunnel);
  if (tunnels.length > 1) fail('Ambiguous tunnel name.');
  let tunnel = tunnels[0];
  if (!tunnel && apply) tunnel = await api(base, 'POST', {name: spec.tunnel, config_src: 'cloudflare'});
  if (!tunnel) fail('Tunnel missing; --apply can provision it.');
  const current = await api(`${base}/${tunnel.id}/configurations`);
  if (apply && (sync || !current.config?.ingress?.length)) {
    await api(`${base}/${tunnel.id}/configurations`, 'PUT', {config: spec.config});
    console.log('Applied version-controlled tunnel routing.');
  }
  for (const name of spec.hosts) {
    const records = await api(`/zones/${zone.id}/dns_records?name=${encodeURIComponent(name)}`);
    const content = `${tunnel.id}.cfargotunnel.com`;
    if (records.length && (records.length !== 1 || records[0].type !== 'CNAME' || records[0].content !== content || !records[0].proxied)) fail(`Conflicting DNS for ${name}; preserved unchanged.`);
    if (!records.length && apply) await api(`/zones/${zone.id}/dns_records`, 'POST', {name, type: 'CNAME', content, proxied: true, ttl: 1});
    console.log(`${name}: ${records.length ? 'configured' : apply ? 'created' : 'missing'}`);
  }
  if (start) {
    const tunnelToken = await api(`${base}/${tunnel.id}/token`);
    const remote = 'set -eu; IFS= read -r PEAKRUNNER_TUNNEL_TOKEN; export PEAKRUNNER_TUNNEL_TOKEN; '
      + `cd ${quote(directory)}; docker compose -f compose.yaml config --quiet; `
      + 'docker network inspect peakrunner-public >/dev/null 2>&1 || docker network create peakrunner-public >/dev/null; '
      + 'docker compose -f compose.yaml up -d tunnel';
    const result = spawnSync('ssh', ['-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes', '-o', 'ConnectTimeout=5', host, remote], {
      input: tunnelToken + '\n', encoding: 'utf8', timeout: 180000,
    });
    if (result.status !== 0) fail('Compose start failed; remote output withheld for credential safety.');
    console.log('Started Compose tunnel; broad API credential remains on administrator machine.');
  }
  const state = await api(`${base}/${tunnel.id}`);
  console.log(`Tunnel: ${state.name}; status: ${state.status}`);
}
main().catch(error => {
  // Filesystem/fetch errors can include sensitive paths; print only our own errors.
  console.error(error.constructor === Error ? error.message : 'Operation failed; details withheld.');
  process.exitCode = 1;
});
