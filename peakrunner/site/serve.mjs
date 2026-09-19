// Local preview only. Production uses the separate Caddy container.
import http from 'node:http';
import {readFile} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('./public/',import.meta.url));
const types={'.html':'text/html','.js':'text/javascript','.css':'text/css','.svg':'image/svg+xml','.png':'image/png','.json':'application/json'};
http.createServer(async(req,res)=>{
  try {
    const url=new URL(req.url,'http://localhost');
    if(url.pathname.startsWith('/api/servers')){
      const upstream=await fetch(`https://dir.peakrunner.net${url.pathname.slice(4)}`,{signal:AbortSignal.timeout(6000)});
      res.writeHead(upstream.status,{'content-type':'application/json','cache-control':'no-store'});res.end(await upstream.text());return;
    }
    const target=path.resolve(root,'.'+decodeURIComponent(url.pathname==='/'?'/index.html':url.pathname));
    if(!target.startsWith(root)||url.pathname.includes('..')){res.writeHead(403);res.end();return;}
    const data=await readFile(target);res.writeHead(200,{'content-type':types[path.extname(target)]||'application/octet-stream','cache-control':'no-store'});res.end(data);
  }catch{res.writeHead(404);res.end('Not found');}
}).listen(8080,'0.0.0.0',()=>console.log('PeakRunner site preview ready'));
