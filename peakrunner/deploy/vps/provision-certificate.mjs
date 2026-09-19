// Administrator-side only. The broad token is never sent to the VPS.
import fs from 'node:fs';
import {spawnSync} from 'node:child_process';
const host = 'root@198.12.80.145';
const ssh = (command, input) => spawnSync('ssh', ['-o','BatchMode=yes','-o','StrictHostKeyChecking=yes',host,command], {input, encoding:'utf8', timeout:180000});
async function main() {
  if (!process.env.CF_API_TOKEN_FILE) throw Error('Set CF_API_TOKEN_FILE.');
  const source = fs.readFileSync(process.env.CF_API_TOKEN_FILE, 'utf8').trim();
  const token = source.match(/Bearer\s+([A-Za-z0-9_-]+)/i)?.[1] ?? source;
  const api = async (path, method='GET', body) => {
    const r = await fetch('https://api.cloudflare.com/client/v4'+path, {method,headers:{Authorization:'Bearer '+token,'Content-Type':'application/json'},body:body?JSON.stringify(body):undefined,signal:AbortSignal.timeout(20000)});
    const d = await r.json(); if (!r.ok || !d.success) throw Error('Cloudflare API failed: '+r.status); return d.result;
  };
  const probe = ssh('test -s /etc/letsencrypt/cloudflare.ini');
  if (probe.status !== 0 && probe.status !== 1) throw Error('SSH credential check failed.');
  if (probe.status === 1) {
    const zones = await api('/zones?name=peakrunner.net');
    if (zones.length !== 1) throw Error('Expected one zone.');
    const z = zones[0], base = '/accounts/'+z.account.id+'/tokens';
    const name = 'peakrunner-vps-certbot';
    if ((await api(base)).some(t=>t.name===name)) throw Error('Renewal token already exists but remote credentials are missing; recover or explicitly revoke it before retrying.');
    const groups = await api(base+'/permission_groups');
    const permissions = ['DNS Write','Zone Read'].map(name=>{
      const g=groups.find(g=>g.name===name && g.scopes.includes('com.cloudflare.api.account.zone'));
      if(!g)throw Error('Required permission unavailable.');return {id:g.id};
    });
    const credential = await api(base,'POST',{name,policies:[{effect:'allow',permission_groups:permissions,resources:{['com.cloudflare.api.account.zone.'+z.id]:'*'}}]});
    const saved=ssh('install -d -m 700 /etc/letsencrypt; umask 077; install -m 600 /dev/stdin /etc/letsencrypt/cloudflare.ini', 'dns_cloudflare_api_token = '+credential.value+'\n');
    if(saved.status!==0)throw Error('Credential transfer failed; recover/revoke the new named token.');
    console.log('Installed zone-scoped renewal credential; secret withheld.');
  }
  const issued=ssh('certbot certonly --dns-cloudflare --dns-cloudflare-credentials /etc/letsencrypt/cloudflare.ini --dns-cloudflare-propagation-seconds 30 --cert-name play.peakrunner.net -d play.peakrunner.net --non-interactive --agree-tos --register-unsafely-without-email --keep-until-expiring');
  if(issued.status!==0)throw Error('Certificate issuance failed; inspect redacted certbot diagnostics on the host.');
  console.log('Public certificate provisioned for play.peakrunner.net.');
}
main().catch(()=>{ console.error('Certificate provisioning failed; credentials and remote output withheld.'); process.exitCode=1; });
