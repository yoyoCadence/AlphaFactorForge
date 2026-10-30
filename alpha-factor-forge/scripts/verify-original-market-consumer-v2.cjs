// Independent, read-only Node built-ins verifier; no product imports/execution.
// Run from any cwd: node alpha-factor-forge/scripts/verify-original-market-consumer-v2.cjs
// Optional argv[2] is an alternative artifact for negative verifier controls.
const fs=require('node:fs'),crypto=require('node:crypto'),assert=require('node:assert/strict'),path=require('node:path');
const dir=path.resolve(__dirname,'../fixtures/research');
const root=path.resolve(__dirname,'../..');
const {execFileSync}=require('node:child_process');
const read=n=>JSON.parse(fs.readFileSync(path.join(dir,n),'utf8').replace(/^\uFEFF/,''));
const bytes=s=>Buffer.byteLength(s,'utf8'),hash=s=>crypto.createHash('sha256').update(s).digest('hex');
const ordered=v=>Array.isArray(v)?v.map(ordered):v&&typeof v==='object'?Object.fromEntries(Object.keys(v).sort().map(k=>[k,ordered(v[k])])):v;
const json=v=>JSON.stringify(ordered(v)),count=v=>1+(Array.isArray(v)?v.reduce((n,x)=>n+count(x),0):v&&typeof v==='object'?Object.values(v).reduce((n,x)=>n+count(x),0):0);
const u32=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n);return b;};
const text=s=>Buffer.concat([u32(bytes(s)),Buffer.from(s)]);
function binary(v){
 if(v===null)return Buffer.from([0]);if(v===false)return Buffer.from([1]);if(v===true)return Buffer.from([2]);
 if(typeof v==='number'){assert(Number.isFinite(v));let b=Buffer.alloc(9);b[0]=3;b.writeDoubleBE(v===0?0:v,1);return b;}
 if(typeof v==='string')return Buffer.concat([Buffer.from([4]),text(v)]);
 if(Array.isArray(v))return Buffer.concat([Buffer.from([5]),u32(v.length),...v.map(binary)]);
 let keys=Object.keys(v).sort((a,b)=>Buffer.compare(Buffer.from(a),Buffer.from(b)));
 return Buffer.concat([Buffer.from([6]),u32(keys.length),...keys.flatMap(k=>[text(k),binary(v[k])])]);
}

function seedVector(rootSeed,oldDataset,newDataset,strategyHash){
 const frame=ds=>Buffer.concat([Buffer.from('seed-v1\0'),u32(rootSeed),text(ds),text(strategyHash),text('random-entry')]);
 const before=frame(oldDataset),after=frame(newDataset);assert.equal(before.length,195);assert.equal(after.length,195);
 const old=crypto.createHash('sha256').update(before).digest().readUInt32BE(0),digest=crypto.createHash('sha256').update(after).digest();
 const next=digest.readUInt32BE(0);
 return {strategyHash,priorRandomEntry:old,randomEntry:next,preimageHex:after.toString('hex'),sha256:digest.toString('hex'),decimalWidthDelta:String(next).length-String(old).length};
}
function enumerate(config){
 const found=new Set();
 for(const base of config.bases){
  let rows=[structuredClone(base.strategy)];
  for(const axis of base.axes){const expanded=[];for(const row of rows){for(let n=axis.min;n<=axis.max;n+=axis.step){assert(Number.isSafeInteger(n));expanded.push({...row,[axis.key]:n});}}rows=expanded;}
  for(const definition of rows){
   if(definition.mode!=='dsl'&&!(definition.fastMA<definition.slowMA&&definition.macdFast<definition.macdSlow&&definition.rsiBuy<definition.rsiSell))continue;
   const payload={definition,execModel:{feePct:definition.feePct,slippagePct:definition.slipPct}};
   found.add('strategy-v2:'+hash(Buffer.concat([Buffer.from('strategy-v2\0'),binary(payload)])));
  }
 }
 return [...found].sort();
}
const v=process.argv[2]?JSON.parse(fs.readFileSync(process.argv[2],'utf8')):read('original-market-consumer-v2.json'),source=read('original-market-source-projections-v1.json'),registry=read('original-market-registry-projections-v1.json'),warm=read('original-warmup-consumer-v1.json'),lineage=read('original-lineage-semantics-v1.json');
assert.equal(v.version,'original-market-consumer-v2');assert.equal(v.cases.length,4);assert.equal(v.resourceBoundaryRecipes.length,4);assert.equal(v.sourceFixtures.length,6);
for(const p of v.sourceFixtures)assert.equal(hash(fs.readFileSync(path.join(dir,p.path),'utf8').replace(/^\uFEFF/,'').replace(/\r\n/g,'\n')),p.lfSha256);
const prior=read('original-market-consumer-v1.json');
const correction=v.seedCorrection;
assert.equal(hash(fs.readFileSync(path.join(dir,correction.supersedes.path),'utf8').replace(/\r\n/g,'\n')),correction.supersedes.lfSha256);
const named=(a,id)=>{let c=a.find(c=>c.id===id);assert(c,id);return c;};
for(const c of v.cases){
 const s=named(source.snapshots,c.snapshotCase),e=s.expected,inst=named(registry.instruments,c.instrumentCase).expected,cal=named(registry.calendars,c.calendarCase).expected,ds=named(source.datasets,c.datasetCase),f=c.family,base=named(warm.cases,f.sourceCase);
 assert.equal(f.datasetHash,ds.metadata.datasetHash);assert.equal(f.datasetRowId,ds.rowId);assert.equal(f.interval,ds.metadata.interval);
 let cfg=JSON.parse(f.configJson);assert.deepEqual(cfg.dataset,{id:ds.rowId,contentHash:ds.metadata.datasetHash});assert.equal(hash(json(cfg)),f.configHash);assert.equal(hash(f.configJson),f.configRawSha256);
 let oldcfg=JSON.parse(base.configJson);delete cfg.dataset;delete oldcfg.dataset;assert.deepEqual(cfg,oldcfg);
 assert.equal(bytes(f.configJson),bytes(base.configJson));assert.deepEqual(f.priorWarmupWork,Object.fromEntries(Object.entries(base.warmupWork).filter(([k])=>['hashBytes','copyBytes','analysisUnits','cloneNodes','outputBytes'].includes(k))));
 const hmap=new Map();
 for(let i=0;i<f.hypotheses.length;i++){
  const h=f.hypotheses[i],b=base.hypotheses[i],d=JSON.parse(b.documentJson);
  Object.assign(d.applicability,{datasetHash:ds.metadata.datasetHash,datasetId:ds.rowId,interval:f.interval});
  assert.equal(json(d),h.contentJson);assert.equal(hash(h.contentJson),h.contentHash);assert.equal(json(d.applicability),h.applicabilityJson);assert.equal(bytes(h.contentJson),bytes(b.documentJson));hmap.set(b.hash,h.contentHash);
 }
 for(let i=0;i<f.inputs.length;i++){
  let d=JSON.parse(base.candidates[i].inputFingerprintJson);d.seeds.randomEntry=seedVector(d.rootSeed,d.datasetHash,f.datasetHash,d.strategyHash).randomEntry;Object.assign(d,{datasetHash:ds.metadata.datasetHash,datasetId:ds.rowId,interval:f.interval,configHash:f.configHash});
  assert.deepEqual(JSON.parse(f.inputs[i].inputFingerprintJson),d);assert.equal(hash(f.inputs[i].inputFingerprintJson),f.inputs[i].sha256);assert.equal(bytes(f.inputs[i].inputFingerprintJson)-bytes(base.candidates[i].inputFingerprintJson),correction.familyVectors.find(x=>x.id===c.id).candidates[i].decimalWidthDelta);
  assert.deepEqual(f.expectedLineage.candidates[i],{...base.expectedObservation.candidates[i],hypothesisHash:hmap.get(base.expectedObservation.candidates[i].hypothesisHash),hypothesisId:base.expectedFreshHistoryReferences[i].hypothesisId});
 }
 assert.equal(f.expectedLineage.configHash,f.configHash);assert.deepEqual(f.expectedWarmup,base.expectedWarmup);
 const camp=JSON.parse(f.campaignDocumentJson),ci=camp.instruments[0];assert.equal(ci.snapshotId,e.snapshotId);assert.equal(ci.datasetHash,ds.metadata.datasetHash);assert.equal(ci.instrumentId,e.scalars.instrument_id);assert.equal(ci.fromMs,ds.metadata.startTime);assert.equal(ci.toMs,ds.metadata.endTime);assert(ci.samplePolicy.foldCount>=2);assert.equal(binary(camp).toString('hex'),f.campaignIdentityPayloadHex);assert.equal(hash(Buffer.concat([Buffer.from('research-campaign-declaration-v1\0'),binary(camp)])),f.campaignId);
 const components=e.sourceRows.map(id=>source.provenance.find(p=>p.expectedRow===id));assert(components.every(Boolean));
 const dsrow=ds.metadata,rows=[{...e.scalars,content_json:e.contentJson},{...inst.scalars,content_json:inst.contentJson,content_hash:inst.contentHash},{...cal.scalars,content_hash:cal.contentHash,definition_json:cal.definitionJson},dsrow,...components.map(p=>({...p.expected.scalars,record_hash:p.expected.recordHash}))];
 const originals=[['snapshot.coverage_json',e.scalars.coverage_json],['snapshot.content_json',e.contentJson],['instrument.suspensions_json',inst.scalars.suspensions_json],['instrument.source_capabilities_json',inst.scalars.source_capabilities_json],['instrument.content_json',inst.contentJson],['calendar.definition_json',cal.definitionJson],...components.map(p=>[`provenance[${p.expectedRow}].request_scope_json`,p.expected.scalars.request_scope_json])];
 const identities=[['calendar',cal.definitionJson],['instrument',inst.contentJson],['snapshot',e.contentJson],...components.map(p=>[`provenance[${p.expectedRow}]`,p.expected.identityJson])];
 let D=0,Nd=0,Jd=0,Ji=0,Ni=0,T=0;const manifest=[];
 rows.forEach((row,i)=>Object.entries(row).forEach(([k,x])=>{if(typeof x==='string'){T+=bytes(x);manifest.push([String(i),k,bytes(x)]);}}));assert.deepEqual(c.retainedStringManifest,manifest);
 originals.forEach(([field,j],i)=>{let n=count(JSON.parse(j)),l=bytes(j);D+=l;Nd+=n;Jd+=bytes(json(JSON.parse(j)));assert.deepEqual(c.originalDocuments[i],{field,utf8Bytes:l,nodes:n,compactBytes:bytes(json(JSON.parse(j))),sha256:hash(j)});});
 identities.forEach(([field,j],i)=>{let n=count(JSON.parse(j)),l=bytes(j);Ji+=l;Ni+=n;assert.deepEqual(c.identityDocuments[i],{field,canonicalJson:j,utf8Bytes:l,nodes:n,sha256:hash(j)});});
 const m=components.length,o=192+72*m,M={originalBytes:D,originalNodes:Nd,retainedTextBytes:T,documentCompactBytes:Jd,identityCompactBytes:Ji,identityNodes:Ni,sourceCount:m,hashBytes:Ji,copyBytes:32*(T+Jd+Ji)+4096*(Nd+Ni)+8192*(m+4)+o,analysisUnits:128*(T+Jd+Ji)+4096*(Nd+Ni)+8192*(m+4),cloneNodes:4*Nd+2*Ni,outputBytes:o};assert.deepEqual(c.marketWork,M);
 assert.deepEqual(c.expectedMarket,{snapshotId:e.snapshotId,instrumentRevisionHash:inst.contentHash,calendarHash:cal.contentHash,sources:components.map(p=>({provenanceId:p.expectedRow,recordHash:p.expected.recordHash}))});
 for(const k of Object.keys(f.priorWarmupWork))assert.equal(c.jointWork[k],f.priorWarmupWork[k]+M[k]);
 for(const k of Object.keys(f.priorPreparation))assert.equal(c.jointPreparation[k],f.priorPreparation[k]+(/^(parse|scratch)/.test(k)?k.endsWith('Nodes')?Nd:D:0));
 assert.equal(hash(e.contentJson),e.snapshotId);assert.equal(e.scalars.status,'ok');assert.notEqual(e.scalars.kind,'demo');assert.equal(e.coverage.observedCount,ds.metadata.candleCount);assert.equal(e.coverage.matchedCount,ds.metadata.candleCount);
 const ids=JSON.parse(e.contentJson);assert.deepEqual(ids.sourceRecordHashes,components.map(p=>p.expected.recordHash).sort());assert.equal(ids.instrumentContentHash,inst.contentHash);
 for(const p of components){const z=p.expected.scalars;assert.equal(z.accepted,1);assert.equal(z.role,'primary');assert.equal(hash(p.expected.identityJson),p.expected.recordHash);assert.equal(z.raw_artifact_path,z.raw_sha256.slice(0,2)+'/'+z.raw_sha256+'.'+({'application/json':'json','text/csv':'csv','text/plain':'txt','application/zip':'zip'})[z.raw_media_type]);}
}
const M=v.cases[0].marketWork,L={hashBytes:33554432,copyBytes:67108864,analysisUnits:268435456,cloneNodes:262144,outputBytes:4194304};
for(const r of v.resourceBoundaryRecipes){
 for(const k of Object.keys(L)){assert.equal(r.exactJoint[k],r.exactPriorWarmup[k]+M[k]);assert(r.exactJoint[k]<=L[k]);}
 const target={hash:'hashBytes',copy:'copyBytes',analysis:'analysisUnits',output:'outputBytes'}[r.kind];assert.equal(r.exactJoint[target],L[target]);assert.equal(r.overJoint[target],L[target]+1);assert(r.exactJointPreparation.scratchBytes<=33554432);assert(r.exactJointPreparation.scratchNodes<=262144);
 if(r.kind==='hash'){assert.equal(r.storedFirstEntryCodeShortenBytes,M.hashBytes);assert.equal(r.exactJoint.copyBytes,65326944-2*M.hashBytes+M.copyBytes);}
 if(r.kind==='analysis'){const old=warm.resourceBoundaryRecipes.find(x=>x.kind==='analysis'),d=old.asciiStoredDslNameBytes-r.storedDslNameBytes,shift=r.paramsEntrySigSuffixBytes-old.paramsStoredEntrySigSuffixBytes;assert.equal(r.exactJoint.analysisUnits,268435456-128*d+shift+M.analysisUnits);assert.equal(r.exactJoint.copyBytes,42061092-20*d+6*shift+M.copyBytes);}
 if(r.kind==='copy'){const old=warm.resourceBoundaryRecipes.find(x=>x.kind==='copy'),d=old.asciiStoredDslNameBytes-r.storedDslNameBytes,shift=r.tenReplacements-old.tenReplacements;assert.equal(r.exactJoint.copyBytes,67108864-20*d+17*shift+String(r.wrapperResourceCounter).length-1+M.copyBytes);assert.equal(String(r.overWrapperResourceCounter).length,String(r.wrapperResourceCounter).length+1);}
 if(r.kind==='output'){assert.equal(r.otherIdBytes,876-M.outputBytes);assert.equal(r.exactJoint.outputBytes,64+1100*(140+3648)+140+r.otherIdBytes+24*1101+M.outputBytes);assert.equal(r.phase,'exact whole observation succeeds; one-over refuses before payload/analysis');}
}
assert.equal(v.coherentRefusals.length,5);
for(const r of v.coherentRefusals){
 const content=JSON.parse(r.contentJson),ps=r.sourceRows.map(id=>source.provenance.find(p=>p.expectedRow===id));
 assert.equal(hash(r.contentJson),r.snapshotId);assert.deepEqual(content.sourceRecordHashes,ps.map(p=>p.expected.recordHash).sort());assert.equal(content.kind,r.kind);assert.equal(r.expected,'original_market_invalid');
 const camp=JSON.parse(r.campaignDocumentJson);assert.equal(camp.instruments[0].snapshotId,r.snapshotId);assert.equal(hash(Buffer.concat([Buffer.from('research-campaign-declaration-v1\0'),binary(camp)])),r.campaignId);
 if(r.id==='late-rejected'){assert.deepEqual(r.sourceRequestOrder,[7,4]);assert(ps.some(p=>!p.expected.scalars.accepted));}
 if(r.id==='comparison')assert.equal(ps[0].expected.scalars.role,'comparison');
 if(r.id==='mixed-publisher')assert(ps.some(p=>p.expected.scalars.source==='coinbase-rest'));
 if(r.id==='forward-legacy-archive')assert.notEqual(JSON.parse(ps[0].expected.scalars.request_scope_json).availabilityBasis,'observed-at-retrieval');
 if(r.id==='forward-late')assert(Date.parse(ps[0].expected.scalars.available_at)>r.asOfMs);
}
for(const [p,h] of Object.entries(v.originalSourcePins))assert.equal(hash(execFileSync('git',['show',correction.originalSourcePinsBaseline+':'+p],{cwd:root,encoding:'utf8'}).replace(/^\uFEFF/,'').replace(/\r\n/g,'\n')),h);
for(const [p,h] of Object.entries(correction.seedSourcePins))assert.equal(hash(fs.readFileSync(path.join(root,p),'utf8').replace(/\r\n/g,'\n')),h);

assert.equal(correction.familyVectors.length,4);
assert.equal(correction.resourceManifests.length,4);
assert.equal(correction.originalSourcePinsBaseline,prior.baseline);
assert.equal(correction.supersedes.path,'original-market-consumer-v1.json');
assert.equal(correction.supersedes.lfSha256,'f9127cc5c0abb09452f6eff721bd95137df5fa03061f7e325ff7772827f1beb8');
assert.deepEqual(Object.keys(correction.seedSourcePins).sort(),['enumerate','identity','seed'].map(n=>'alpha-factor-forge/src-tauri/src/discovery_core/'+n+'.rs'));
const normalized=structuredClone(v);delete normalized.seedCorrection;
normalized.version=prior.version;normalized.baseline=prior.baseline;
for(let i=0;i<v.cases.length;i++){
 const c=v.cases[i],f=c.family,manifest=correction.familyVectors[i],base=named(warm.cases,f.sourceCase),cfg=JSON.parse(f.configJson);
 const before=JSON.parse(base.configJson).dataset.contentHash,identities=enumerate(cfg);
 const vectors=f.inputs.map((item,index)=>{
  const input=JSON.parse(item.inputFingerprintJson);assert.equal(input.strategyHash,identities[index]);
  const vector=seedVector(cfg.rootSeed,before,cfg.dataset.contentHash,identities[index]);
  assert.equal(input.seeds.randomEntry,vector.randomEntry);
  const old=prior.cases[i].family.inputs[index].inputFingerprintJson;
  assert.equal(JSON.parse(old).seeds.randomEntry,vector.priorRandomEntry);
  assert.equal(item.inputFingerprintJson,old.replace('"randomEntry":'+vector.priorRandomEntry+'}','"randomEntry":'+vector.randomEntry+'}'));
  normalized.cases[i].family.inputs[index]=prior.cases[i].family.inputs[index];
  return {index,...vector};
 });
 const delta=vectors.reduce((n,x)=>n+x.decimalWidthDelta,0);
 assert.deepEqual(manifest,{id:c.id,priorDatasetHash:before,datasetHash:cfg.dataset.contentHash,rootSeed:cfg.rootSeed,parseByteDelta:delta,candidates:vectors});
 for(const key of ['parseBytes','scratchBytes']){
  normalized.cases[i].family.priorPreparation[key]-=delta;
  normalized.cases[i].jointPreparation[key]-=delta;
 }
}
const producers=read('original-producer-projections-v1.json');
const template=id=>JSON.parse(named(producers.cases,id).configJson);
const pinnedManifests={hash:'0736baeba835ba4944e569d9f3d7c6c1849776cd00dcc416bde6058583afeae9',analysis:'a928ab15d6b42e08192466cc7496f8823df64b623ef905236ce91368062861e2',copy:'2b8e28754ce1b334bb77b67b1b07120a51cbfd0cf2280aad6cf4779e5891efd7',output:'d85096b806a67b89fda894381e690d67764d2f568017ce4e7174813605257990'};
for(let i=0;i<v.resourceBoundaryRecipes.length;i++){
 const recipe=v.resourceBoundaryRecipes[i],kind=recipe.kind,m=correction.resourceManifests[i];
 assert.equal(m.kind,kind);
 const expected=template(['hash','output'].includes(kind)?'v1-ordered-axes':'v2-fixed-dsl');
 const before=expected.dataset.contentHash;
 expected.dataset={id:1,contentHash:v.cases[0].family.datasetHash};expected.caps.candidates=4096;
 if(kind==='hash'){
  expected.bases[0].id='resource-main';expected.bases[0].axes[0]={key:'fastMA',min:1,max:8,step:1};
 }else if(kind==='output'){
  const reference=warm.outputBoundaryRecipe,b=expected.bases[0];
  b.id='m'.repeat(recipe.mainIdBytes);b.strategy.slowMA=reference.mainSlowMA;b.axes=reference.mainAxes;
  const other=structuredClone(b);other.id='o'.repeat(recipe.otherIdBytes);other.strategy.emaPeriod=reference.otherEmaPeriod;other.axes=[];expected.bases.push(other);
 }else{
  const reference=warm.resourceBoundaryRecipes.find(r=>r.kind===kind);
  expected.bases[0].id=reference.baseId;expected.bases[0].strategy.dsl.name=reference.baseDslName;
  if(kind==='analysis'){
   const sidecar=JSON.parse(named(warm.cases,'v1-varied-slow').configJson).bases[0];sidecar.id=reference.paramsSidecar.id;sidecar.axes=[];expected.bases.push(sidecar);
  }
 }
 assert.deepEqual(JSON.parse(m.originalConfigJson),expected);assert.equal(m.originalConfigJson,json(expected));assert.equal(m.originalConfigHash,hash(json(expected)));
 assert.equal(m.priorDatasetHash,before);assert.equal(m.datasetHash,expected.dataset.contentHash);assert.equal(m.rootSeed,expected.rootSeed);
 const identities=enumerate(expected),vectors=identities.map((h,index)=>({index,...seedVector(expected.rootSeed,before,expected.dataset.contentHash,h)}));
 assert.deepEqual(m.candidates,vectors);assert.equal(m.candidateCount,identities.length);
 assert.equal(m.candidateCount,{hash:8,analysis:2,copy:1,output:1101}[kind]);
 assert.equal(m.priorSeedDecimalBytes,vectors.reduce((n,x)=>n+String(x.priorRandomEntry).length,0));
 assert.equal(m.seedDecimalBytes,vectors.reduce((n,x)=>n+String(x.randomEntry).length,0));
 assert.equal(m.parseByteDelta,m.seedDecimalBytes-m.priorSeedDecimalBytes);
 const lines=vectors.map(x=>`${x.strategyHash} ${x.priorRandomEntry} ${x.randomEntry}\n`).join('');
 assert.equal(m.sortedSeedLinesSha256,hash(lines));assert.equal(hash(lines),pinnedManifests[kind]);
 for(const key of ['parseBytes','scratchBytes'])for(const variant of ['exactJointPreparation','overJointPreparation'])normalized.resourceBoundaryRecipes[i][variant][key]-=m.parseByteDelta;
 if(kind==='output'){
  const over=structuredClone(expected);over.bases[1].id+='o';assert.deepEqual(enumerate(over),identities);
 }
}
assert.equal(correction.familyVectors.reduce((n,m)=>n+m.candidates.length,0),14);
assert.equal(correction.resourceManifests.reduce((n,m)=>n+m.candidateCount,0),1112);
// This exact comparison forbids unrelated changes hidden behind seed correction.
assert.deepEqual(normalized,prior);
console.log('PASS: all14 family/1112 resource seeds independently rederived; original fingerprints/token formatting, preparation deltas, original catalogs/market work/identities, source pins and v1 preservation checked');
