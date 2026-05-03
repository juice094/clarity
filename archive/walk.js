const fs=require('fs'),path=require('path');
function walk(dir){
  let r=[];
  for(const f of fs.readdirSync(dir)){
    if(f==='.git') continue;
    const p=path.join(dir,f);
    const s=fs.statSync(p);
    if(s.isDirectory()) r.push(...walk(p));
    else r.push('./'+p.replace(/\\/g,'/'));
  }
  return r;
}
console.log(JSON.stringify(walk('.').sort()));
