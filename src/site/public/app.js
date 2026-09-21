const $ = (id) => document.getElementById(id);
const el = (tag, text, className) => { const n = document.createElement(tag); if (text !== undefined) n.textContent = text; if (className) n.className = className; return n; };
let hosts = [], selected = null, selectedPlayer = null, detailsCache = new Map(), busy = false;
const dialog = $('host-dialog');
async function json(path) { const r = await fetch(path, { cache: 'no-store', signal: AbortSignal.timeout(6000) }); if (!r.ok) throw Error(`HTTP ${r.status}`); return r.json(); }
function stat(label, value) { const n = el('div', undefined, 'stat'); n.append(el('span', label), el('strong', String(value))); return n; }
function showHost() {
  const host = hosts.find(h => h.id === selected), root = $('host-details'); root.replaceChildren();
  $('host-title').textContent = host?.name || 'Host unavailable';
  const s = detailsCache.get(selected);
  if (!host || !s) { root.append(el('p', 'Live details are unavailable. The host may be restarting; refresh in a moment.', 'empty')); return; }
  const grid = el('div', undefined, 'match-grid'); grid.append(stat('MAP',s.map),stat('PLAYERS',`${s.players} / ${s.max_players}`),stat('PHASE',s.phase));
  const scores = el('div', undefined, 'match-grid'); const time = Math.max(0,Math.floor(s.time_left));
  scores.append(stat('EMBER',s.score[0]),stat('GLACIER',s.score[1]),stat('REMAINING',`${Math.floor(time/60)}:${String(time%60).padStart(2,'0')}`));
  root.append(grid,scores,el('p',`Build ${s.game_version || 'not reported'}`, 'version'),el('p',`Compatibility: ${s.game_protocol || 'not reported'}`, 'version'));
  const address = el('div', undefined, 'address-row'), copy = el('button','Copy address','button small');
  copy.addEventListener('click',async () => { try { await navigator.clipboard.writeText(host.host); copy.textContent='Copied'; } catch { copy.textContent='Select the address to copy'; } });
  address.append(el('code',host.host),copy); root.append(address,el('h3','Player roster'));
  const roster = Array.isArray(s.roster) ? s.roster : [];
  if (!roster.length) root.append(el('p','No players yet. The next drop is yours.','empty'));
  else {
    const table = el('table',undefined,'roster'), head=el('thead'), tr=el('tr');
    for(const title of ['Player','Team','Frags','Deaths']) tr.append(el('th',title)); head.append(tr); table.append(head);
    const body=el('tbody');
    for(const p of roster) { const row=el('tr'), cell=el('td'), button=el('button',p.name,'player-button'); button.addEventListener('click',()=>{selectedPlayer=p.id;showPlayer(roster);}); cell.append(button); row.append(cell,el('td',p.team),el('td',String(p.frags)),el('td',String(p.deaths))); body.append(row); }
    table.append(body);root.append(table,el('div',undefined,'player-detail')); root.lastChild.id='player-detail'; showPlayer(roster);
  }
  root.append(el('p','Names are player-chosen, not verified accounts. Scores reset with the match.','subtle'));
}
function showPlayer(roster) { const node=$('player-detail'); if(!node)return; const p=roster.find(p=>p.id===selectedPlayer); node.textContent=p?`${p.name} · ${p.team} · ${p.frags} frags / ${p.deaths} deaths · K/D ${p.deaths?(p.frags/p.deaths).toFixed(2):'— (no deaths)'}`:'Click a player to inspect their current-match stats.'; }
function renderHosts() {
  const root=$('server-list'); root.replaceChildren();
  if(!hosts.length) {root.append(el('p','No hosts are currently reporting online. Check back shortly.','empty'));return;}
  for(const host of hosts) {
    const card=el('button',undefined,'host-card'); card.setAttribute('aria-label',`View ${host.name} server details`);
    const img=el('img'); img.src='/assets/raindance.png'; img.alt=''; img.width=180;img.height=136;
    const info=el('span'), status=detailsCache.get(host.id);info.append(el('span',host.name,'host-name'),el('span',`${host.map} · Capture the flag · ${status?.phase || 'Live host'}`,'host-meta'),el('span',`Build ${status?.game_version || 'not reported'}`,'host-meta'));
    const count=el('span',undefined,'host-occupancy');count.append(el('strong',`${host.players}/${host.max_players}`),el('small','View match ↗'));
    card.append(img,info,count);card.addEventListener('click',()=>{selected=host.id;selectedPlayer=null;showHost();dialog.showModal();});root.append(card);
  }
}
async function refresh() {
  if(busy)return;busy=true;$('refresh').disabled=true;
  try { const data=await json('/api/servers'); if(!Array.isArray(data.servers))throw Error('Invalid directory');hosts=data.servers;
    const results=await Promise.all(hosts.map(async h=>{try{return [h.id,await json(`/api/servers/${encodeURIComponent(h.id)}`)];}catch{return [h.id,null];}}));detailsCache=new Map(results);
    renderHosts();const count=hosts.reduce((n,h)=>n+h.players,0);$('network-status').textContent=`${hosts.length} host${hosts.length===1?'':'s'} online / ${count} player${count===1?'':'s'}`;$('updated').textContent=`Updated ${new Date().toLocaleTimeString([],{hour:'2-digit',minute:'2-digit',second:'2-digit'})}`;
  }catch {hosts=[];detailsCache.clear();$('server-list').replaceChildren(el('p','The directory is temporarily unreachable. Downloads are still available. Use Refresh to try again.','empty'));$('network-status').textContent='Directory unavailable';$('updated').textContent='Live status unavailable';}
  finally{busy=false;$('refresh').disabled=false;if(dialog.open)showHost();}
}
async function downloads() {
  try { const release=await json('/release.json');$('release-version').textContent=release.version;
    const root=$('downloads-list');root.replaceChildren();
    for(const d of release.downloads){const card=el('article',undefined,'download-card');card.append(el('p',d.platform,'eyebrow'),el('h3',d.title),el('p',d.description));
      if(d.url && d.url.startsWith('/downloads/')){const a=el('a',d.label,'button');a.href=d.url;a.setAttribute('download','');card.append(a);if(d.sha256)card.append(el('small',`SHA-256: ${d.sha256}`));}
      else card.append(el('p','Binary not yet published','subtle'));root.append(card);}
  }catch{$('release-version').textContent='Release info unavailable';$('downloads-list').replaceChildren(el('p','Downloads could not be loaded. Please refresh this page.','empty'));}
}
$('refresh').addEventListener('click',refresh);$('close-dialog').addEventListener('click',()=>dialog.close());
dialog.addEventListener('click',e=>{if(e.target===dialog){const r=dialog.getBoundingClientRect();if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)dialog.close();}});
refresh();downloads();setInterval(()=>{if(!document.hidden)refresh();},5000);
