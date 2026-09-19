// Test-only localhost UDP impairment proxy; never a production service.
// Clients use quic://play.peakrunner.net:17777 with diagnostic IP 127.0.0.1.
import dgram from 'node:dgram';
const listener=dgram.createSocket('udp4');
const peers=new Map();
let packets=0, dropped=0;
const forward=(socket,packet,port,host)=>{
  packets++;
  if(packets%100===0){dropped++;return;} // deterministic 1% packet loss
  setTimeout(()=>socket.send(packet,port,host),packets%7===0?60:10); // jitter/reordering
};
listener.on('message',(packet,from)=>{
  const id=from.address+':'+from.port;
  let peer=peers.get(id);
  if(!peer){
    if(peers.size>=16)return;
    peer=dgram.createSocket('udp4'); peers.set(id,peer);
    peer.on('message',data=>forward(listener,data,from.port,from.address));
    peer.on('error',()=>{});
  }
  forward(peer,packet,7777,'198.12.80.145');
});
listener.bind(17777,'127.0.0.1',()=>console.log('Local UDP test proxy: 1% loss, 10–60ms added delay each direction'));
const finish=()=>{ console.log(JSON.stringify({packets,dropped})); for(const p of peers.values())p.close();listener.close();process.exit(0); };
process.on('SIGINT',finish);process.on('SIGTERM',finish);
