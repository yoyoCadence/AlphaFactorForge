// Independent read-only Node built-ins oracle. Never import/execute product code.
// Run from any cwd; optional argv[2] is an alternative artifact for refusal controls.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto'),assert=require('node:assert/strict');
const {execFileSync}=require('node:child_process');
const root=path.resolve(__dirname,'../..'),dir=path.resolve(__dirname,'../fixtures/research');
const read=n=>JSON.parse(fs.readFileSync(path.join(dir,n),'utf8').replace(/^\uFEFF/,''));
const v=process.argv[2]?JSON.parse(fs.readFileSync(process.argv[2],'utf8')):read('original-trial-projections-v1.json');
const baseline='bbf669af8f4cda80e63b7e12da42831e32fe8d75',MAX=9007199254740991n;
const bytes=s=>Buffer.byteLength(s,'utf8'),sha=s=>crypto.createHash('sha256').update(s).digest('hex');
const order=(a,b)=>Buffer.compare(Buffer.from(a),Buffer.from(b));
const sorted=x=>Array.isArray(x)?x.map(sorted):x&&typeof x==='object'?Object.fromEntries(Object.keys(x).sort(order).map(k=>[k,sorted(x[k])])):x;
const json=x=>JSON.stringify(sorted(x)),clone=structuredClone;
const keys=(value,expected)=>assert.deepEqual(Object.keys(value).sort(),[...expected].sort());
keys(v,['version','baseline','scope','platform','inputCarrier','sourcePins','sourceFixtures','cases','batchControls','producerValidationRefusals','primitiveSplitControls','dynamicRegistryFieldsNotAuthored','futureCompleteMembershipRefusals']);
const u32=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n);return b;};
const text=s=>Buffer.concat([u32(bytes(s)),Buffer.from(s)]);
function binary(x){
 if(x===null)return Buffer.from([0]);if(x===false)return Buffer.from([1]);if(x===true)return Buffer.from([2]);
 if(typeof x==='number'){assert(Number.isFinite(x));const b=Buffer.alloc(9);b[0]=3;b.writeDoubleBE(x===0?0:x,1);return b;}
 if(typeof x==='string')return Buffer.concat([Buffer.from([4]),text(x)]);
 if(Array.isArray(x))return Buffer.concat([Buffer.from([5]),u32(x.length),...x.map(binary)]);
 const keys=Object.keys(x).sort(order);return Buffer.concat([Buffer.from([6]),u32(keys.length),...keys.flatMap(k=>[text(k),binary(x[k])])]);
}
function strategy(definition){return 'strategy-v2:'+sha(Buffer.concat([Buffer.from('strategy-v2\0'),binary({definition,execModel:{feePct:definition.feePct,slippagePct:definition.slipPct}})]));}
function seed(config,h){
 const frame=Buffer.concat([Buffer.from('seed-v1\0'),u32(config.rootSeed),text(config.dataset.contentHash),text(h),text('random-entry')]);
 assert.equal(frame.length,195);const digest=crypto.createHash('sha256').update(frame).digest();
 return {seeds:{randomEntry:digest.readUInt32BE(0)},preimage:frame.toString('hex'),digest:digest.toString('hex')};
}
const range=(start,count)=>({from:Number(start),to:Number(start+count-1n),count:Number(count)});
function split(total,embargo){
 const n=BigInt(total),e=BigInt(embargo);if(n<0n||e<0n||n>MAX||e>MAX||n-2n*e<5n)return null;
 const usable=n-2n*e,weights=[3n,1n,1n],counts=weights.map(w=>w*(usable/5n)+w*(usable%5n)/5n);
 const remainders=weights.map(w=>w*(usable%5n)%5n),indices=[0,1,2].sort((a,b)=>remainders[a]===remainders[b]?a-b:remainders[a]>remainders[b]?-1:1);
 for(let i=0,left=Number(usable-counts.reduce((a,b)=>a+b,0n));i<left;i++)counts[indices[i]]++;
 const train=range(0n,counts[0]),gap1=e===0n?null:range(counts[0],e),validation=range(counts[0]+e,counts[1]);
 const gap2=e===0n?null:range(BigInt(validation.to)+1n,e),test=range(BigInt(validation.to)+e+1n,counts[2]);assert.equal(test.to,Number(n-1n));
 return {totalBars:total,usableBars:Number(usable),embargoBars:embargo,train,trainValidationEmbargo:gap1,validation,validationTestEmbargo:gap2,test};
}
function walk(total,embargo,d){
 const plan={totalBars:total,embargoBars:embargo,...d};
 if(Object.values(plan).some(n=>BigInt(n)<0n||BigInt(n)>MAX)||d.minimumTrainBars===0||d.foldValidationBars===0||d.foldCount<2||d.foldCount>128)return null;
 const e=BigInt(embargo),count=BigInt(d.foldCount),validation=BigInt(d.foldValidationBars),required=BigInt(d.minimumTrainBars)+count*(e+validation);
 const result={contractVersion:'research-walk-forward-v1',plan,status:'NOT_ELIGIBLE',reasons:[],outerTrain:null,availableTrainBars:null,requiredTrainBars:required>MAX?null:Number(required),folds:[]};
 const outer=split(total,embargo);if(outer===null){result.reasons=['insufficient_outer_history'];return result;}
 const available=BigInt(outer.train.count);result.outerTrain=outer.train;result.availableTrainBars=Number(available);
 if(available<required){result.reasons=['insufficient_train_history'];return result;}
 let cursor=available-count*(e+validation);
 for(let index=0;index<d.foldCount;index++){
  const train=range(0n,cursor),gap=e===0n?null:range(cursor,e);cursor+=e;
  const span=range(cursor,validation);cursor+=validation;result.folds.push({index,train,embargo:gap,validation:span});
 }
 assert.equal(cursor,available);result.status='ELIGIBLE';return result;
}
function identity(config,embargo,total){
 return {contracts:{split:config.contracts.split,embargo:config.contracts.embargo},totalBars:total,holdingAllowanceBars:config.embargo.holdingAllowanceBars,
  embargo,split:embargo===null?null:split(total,embargo.embargoBars),
  walkForward:config.walkForward?{declaration:config.walkForward,report:embargo===null?null:walk(total,embargo.embargoBars,config.walkForward)}:null};
}
const fields=['hypothesisHash','strategyHash','datasetHash','snapshotId','splitHash','seedsHash','engineFingerprintHash','benchmarkId','benchmarkParamsHash','reproductionOf'];
const instruments=new Set(['crypto:binance:BTCUSDT','us-etf:nyse-arca:SPY','tw-etf:twse:0050']);
function validate(b){
 if(b.events.length===0)return 'empty_events';if(!b.workspaceId.trim()||b.workspaceId.includes(':'))return 'workspace';
 if(BigInt(b.testsPerTrial)<1n||BigInt(b.testsPerTrial)>MAX)return 'protocol_count';if(b.instrumentId!==null&&!instruments.has(b.instrumentId))return 'instrument';
 const legacy=b.events[0].origin.type==='legacy',requests=new Set(),keys=new Set();
 for(const e of b.events){
  for(const f of fields){assert(Object.hasOwn(e,f));if(e[f]!==null&&!e[f].trim())return 'blank_'+f;}
  if((e.snapshotId!==null)!==(b.instrumentId!==null))return 'snapshot_presence';
  let key;
  if(e.origin.type==='request'){
   if(legacy||e.kind==='legacy')return 'origin_kind';if(!e.origin.requestId.trim()||e.origin.requestId.includes(':'))return 'request';
   requests.add(e.origin.requestId);key=`${b.workspaceId}:request:${e.origin.requestId}:candidate:${e.origin.candidateIndex}`;
  }else{
   if(!legacy||e.kind!=='legacy')return 'origin_kind';if(!e.origin.attemptKey.trim())return 'attempt_key';key=b.workspaceId+':'+e.origin.attemptKey;
  }
  if(requests.size>1)return 'mixed_request';if(keys.has(key))return 'duplicate_key';keys.add(key);
  assert(['variant','hypothesis','diagnostic'].includes(e.kind));
  for(const f of ['strategyHash','datasetHash','splitHash','seedsHash','engineFingerprintHash'])if(e[f]===null)return 'missing_'+f;
  for(const f of ['benchmarkId','benchmarkParamsHash','reproductionOf'])if(e[f]!==null)return 'forbidden_'+f;
 }
 return null;
}
function batch(b){
 assert.equal(validate(b),null);const familyKeyJson=b.instrumentId===null?null:json({version:'trial-family-v1',instrumentId:b.instrumentId});
 const familyId=familyKeyJson===null?null:'trial-family-v1:'+sha(familyKeyJson);
 const events=b.events.map((e,inputPosition)=>{
  const key=`${b.workspaceId}:request:${e.origin.requestId}:candidate:${e.origin.candidateIndex}`;
  const payload={version:'trial-event-v1',familyId,kind:e.kind,effective:true,idempotencyKey:key,workspaceId:b.workspaceId,requestId:e.origin.requestId,candidateIndex:e.origin.candidateIndex,legacyAttemptKey:null};
  for(const f of fields)payload[f]=e[f];assert.equal(Object.keys(payload).length,19);assert(!Object.hasOwn(payload,'benchmarkEvidence'));
  const payloadJson=json(payload);return {inputPosition,candidateIndex:e.origin.candidateIndex,idempotencyKey:key,payloadJson,eventId:'trial-event-v1:'+sha(payloadJson)};
 });
 const ordered=events.toSorted((a,b)=>order(a.idempotencyKey,b.idempotencyKey));
 const members=ordered.map(e=>[e.idempotencyKey,e.eventId]),batchIdentityJson=json({version:'trial-batch-v1',familyId,members}),batchId='trial-batch-v1:'+sha(batchIdentityJson);
 ordered.forEach((e,i)=>{e.freshSequence=i+1;e.storedProjection={event_id:e.eventId,family_id:familyId,idempotency_key:e.idempotencyKey,kind:JSON.parse(e.payloadJson).kind,effective:1,batch_id:batchId,payload_json:e.payloadJson};});
 return {familyKeyJson,familyId,protocolJson:json({correction:'holm',testsPerTrial:b.testsPerTrial}),batchIdentityJson,batchId,membersJson:json(members),candidateOrderByMembers:ordered.map(e=>e.candidateIndex),returnedEventIdsInputOrder:events.map(e=>e.eventId),events};
}
const market=read('original-market-consumer-v2.json'),warm=read('original-warmup-consumer-v1.json'),producers=read('original-producer-projections-v1.json');
const named=(a,id)=>{const found=a.find(v=>v.id===id);assert(found,id);return found;};
function source(kind,id){
 if(kind==='market'){
  const c=named(market.cases,id),f=c.family,old=named(warm.cases,f.sourceCase);
  return {raw:f.configJson,engine:old.engineFingerprintJson,instrument:JSON.parse(f.campaignDocumentJson).instruments[0].instrumentId,snapshot:c.expectedMarket.snapshotId,
   candidates:old.candidates.map((s,i)=>({raw:s.freshStrategy.original_definition_json,definition:JSON.parse(s.freshStrategy.original_definition_json),hash:f.expectedLineage.candidates[i].strategyHash,hypothesis:f.expectedLineage.candidates[i].hypothesisHash,seeds:JSON.parse(f.inputs[i].inputFingerprintJson).seeds,embargo:f.expectedWarmup[i]}))};
 }
 const c=kind==='overflow'?warm.derivationRefusalCases[0]:named(producers.cases,id);
 const derivations=kind==='overflow'?c.expectedPerCandidate.map(x=>x.derivation??null):warm.existingFamilies.find(x=>x.caseId===id&&x.sourceFixture==='original-producer-projections-v1.json').expectedWarmup;
 return {raw:c.configJson,engine:c.engineFingerprintJson,instrument:null,snapshot:null,
  candidates:c.candidates.map((s,i)=>({raw:s.freshStrategy.original_definition_json,definition:JSON.parse(s.freshStrategy.original_definition_json),hash:s.strategyHash,hypothesis:kind==='overflow'?c.expectedObservation.candidates[i].hypothesisHash:s.hypothesisHash,seeds:JSON.parse(s.inputFingerprintJson).seeds,embargo:derivations[i]}))};
}
const recipes=[['v1-market-small','market','captured-old-revision',4],['v1-market-full','market','captured-old-revision',600],['v1-shared-dedup','producer','v1-shared-dedup',600],['v2-fixed-dsl','producer','v2-fixed-dsl',600],['v3-mixed-lookback-distinction','market','new-revision-mixed',600],['v3-mixed-insufficient-outer','market','new-revision-mixed',4],['v3-dsl-insufficient-train','producer','v3-walk-forward',100],['v3-mixed-null-embargo','overflow','v3-mixed-failed-embargo-overflow',600],['v1-twelve-lexical','grid','v1-ordered-axes',600],['etf-declared-family','market','etf-declared-references',600],['v3-outside-safe-count','producer','v3-walk-forward',Number(MAX+1n)]];
assert.equal(v.version,'original-trial-projections-v1');assert.equal(v.baseline,baseline);assert.equal(v.cases.length,11);assert.equal(v.cases.reduce((n,c)=>n+c.candidates.length,0),41);
for(let position=0;position<recipes.length;position++){
 const [id,kind,sourceId,total]=recipes[position],c=v.cases[position],s=source(kind==='grid'?'producer':kind,sourceId);let config=JSON.parse(s.raw);
 keys(c,['id','sourceKind','sourceCase','countAuthority','declaredCandleCount','configJson','configHash','configRawSha256','engineFingerprintJson','candidates','batchInput','expectedBatch']);
 assert.equal(c.id,id);assert.equal(c.sourceKind,kind);assert.equal(c.sourceCase,sourceId);assert.equal(c.declaredCandleCount,total);assert(c.countAuthority.includes('no VerifiedDataset'));
 if(kind==='grid'){
  config.bases[0].axes[0]={key:'fastMA',min:1,max:12,step:1};s.raw=json(config);
  const definition=config.bases[0].strategy,base=s.candidates[0];assert.equal(definition.slowMA,21);assert.equal(config.embargo.holdingAllowanceBars,10);
  assert.equal(definition.entrySig,'maCrossUp');assert.equal(definition.exitSig,'maCrossDown');
  s.candidates=Array.from({length:12},(_,i)=>{
   const d={...clone(definition),fastMA:i+1,slowMA:21},hash=strategy(d);
   const raw=json(d).replace(/"fastMA":(\d+)(?=[,}])/,'"fastMA":$1.0').replace(/"slowMA":(\d+)(?=[,}])/,'"slowMA":$1.0');
   return {raw,definition:d,hash,hypothesis:base.hypothesis,seeds:null,embargo:{maxSignalLookbackBars:22,holdingAllowanceBars:10,embargoBars:32}};
  }).sort((a,b)=>order(a.hash,b.hash));
 }
 assert.equal(c.configJson,s.raw);assert.equal(c.configRawSha256,sha(s.raw));assert.equal(c.configHash,sha(json(config)));assert.equal(c.engineFingerprintJson,s.engine);assert.equal(json(JSON.parse(s.engine)),s.engine);
 assert.equal(c.candidates.length,s.candidates.length);
 const expectedEvents=[];
 s.candidates.forEach((original,index)=>{
  const candidate=c.candidates[index];assert.equal(strategy(original.definition),original.hash);
  const derived=seed(config,original.hash);if(original.seeds!==null)assert.deepEqual(derived.seeds,original.seeds);
  const doc=identity(config,original.embargo,total),payload=json({version:'trial-split-v1',identity:doc}),splitHash='trial-split-v1:'+sha(payload);
  let check={status:'not_applicable'};
  if(config.walkForward){
   if(original.embargo===null)check={status:'refused',phase:'embargo'};
   else if(config.walkForward.minimumTrainBars<original.embargo.maxSignalLookbackBars)check={status:'refused',phase:'candidate_lookback'};
   else if(doc.walkForward.report===null)check={status:'refused',phase:'walk_forward_plan'};
   else check={status:'ok',report:doc.walkForward.report};
  }
  assert.deepEqual(candidate,{index,strategyHash:original.hash,hypothesisHash:original.hypothesis,definitionJson:original.raw,seedsJson:json(derived.seeds),seedPreimageHex:derived.preimage,seedSha256:derived.digest,seedsHash:sha(json(derived.seeds)),engineFingerprintHash:sha(s.engine),identityJson:json(doc),splitIdentityPayloadJson:payload,splitHash,candidateWalkForwardReport:check});
  assert.equal(Object.keys(doc).length,6);
  expectedEvents.push({kind:'variant',origin:{type:'request',requestId:'request-'+id,candidateIndex:index},hypothesisHash:original.hypothesis,strategyHash:original.hash,datasetHash:config.dataset.contentHash,snapshotId:s.snapshot,splitHash,seedsHash:sha(json(derived.seeds)),engineFingerprintHash:sha(s.engine),benchmarkId:null,benchmarkParamsHash:null,reproductionOf:null});
 });
 const input={workspaceId:'trial-projection-workspace',instrumentId:s.instrument,testsPerTrial:2,events:expectedEvents};
 assert.deepEqual(c.batchInput,input);assert.deepEqual(c.expectedBatch,batch(input));
}
const grid=named(v.cases,'v1-twelve-lexical');assert.deepEqual(grid.expectedBatch.candidateOrderByMembers,[0,1,10,11,2,3,4,5,6,7,8,9]);assert.equal(new Set(grid.candidates.map(c=>c.splitHash)).size,1);assert.equal(new Set(grid.candidates.map(c=>c.strategyHash)).size,12);
const controls=['reversed-input','raw-spaces','unversioned-split','absent-hypothesis','diagnostic-kind','protocol-three'];assert.equal(v.batchControls.length,controls.length);
v.batchControls.forEach((c,i)=>{
 keys(c,['id','baseCase','ledgerPreparation','meaning','batchInput','expectedBatch']);
 assert.equal(c.id,controls[i]);assert.equal(c.baseCase,grid.id);const input=clone(grid.batchInput);
 if(c.id==='reversed-input')input.events.reverse();
 else if(c.id==='raw-spaces'){input.workspaceId=' workspace-\u00e9 ';input.events.forEach(e=>e.origin.requestId=' request-\u00e9 ');}
 else if(c.id==='unversioned-split')input.events[0].splitHash='unversioned-original-claim';
 else if(c.id==='absent-hypothesis')input.events[0].hypothesisHash=null;
 else if(c.id==='diagnostic-kind')input.events[0].kind='diagnostic';
 else input.testsPerTrial=3;
 assert.equal(c.ledgerPreparation,'accepted');assert.deepEqual(c.batchInput,input);assert.deepEqual(c.expectedBatch,batch(input));
});
assert.equal(named(v.batchControls,'reversed-input').expectedBatch.batchId,grid.expectedBatch.batchId);
assert.deepEqual(named(v.batchControls,'reversed-input').expectedBatch.returnedEventIdsInputOrder,grid.expectedBatch.returnedEventIdsInputOrder.toReversed());
assert.equal(named(v.batchControls,'protocol-three').expectedBatch.batchId,grid.expectedBatch.batchId);
assert.notEqual(named(v.batchControls,'protocol-three').expectedBatch.protocolJson,grid.expectedBatch.protocolJson);
const refusalRecipes=[
 ['empty-events',b=>b.events=[],'empty_events'],['empty-workspace',b=>b.workspaceId=' ','workspace'],['workspace-colon',b=>b.workspaceId='a:b','workspace'],
 ['zero-protocol',b=>b.testsPerTrial=0,'protocol_count'],['protocol-over-safe',b=>b.testsPerTrial=Number(MAX+1n),'protocol_count'],['bad-instrument',b=>b.instrumentId='bad','instrument'],
 ['snapshot-without-instrument',b=>b.instrumentId=null,'snapshot_presence'],['missing-snapshot',b=>b.events[0].snapshotId=null,'snapshot_presence'],
 ['empty-request',b=>b.events[0].origin.requestId=' ','request'],['request-colon',b=>b.events[0].origin.requestId='a:b','request'],
 ['mixed-request',b=>b.events[1].origin.requestId='another','mixed_request'],['duplicate-index-key',b=>b.events[1].origin.candidateIndex=0,'duplicate_key'],
 ['legacy-kind-request-origin',b=>b.events[0].kind='legacy','origin_kind'],['legacy-origin-variant-kind',b=>b.events[0].origin={type:'legacy',attemptKey:'old'},'origin_kind'],
 ['blank-hypothesis',b=>b.events[0].hypothesisHash=' ','blank_hypothesisHash'],
 ...['strategyHash','datasetHash','splitHash','seedsHash','engineFingerprintHash'].map(f=>['missing-'+f,b=>b.events[0][f]=null,'missing_'+f]),
 ...['benchmarkId','benchmarkParamsHash','reproductionOf'].map(f=>['forbidden-'+f,b=>b.events[0][f]='claimed','forbidden_'+f]),
 ['blank-strategy',b=>b.events[0].strategyHash=' ','blank_strategyHash'],
];
assert.equal(v.producerValidationRefusals.length,24);
v.producerValidationRefusals.forEach((r,i)=>{const [id,change,reason]=refusalRecipes[i],input=clone(v.cases[0].batchInput);change(input);assert.deepEqual(r,{id,batchInput:input,expectedCode:'invalid_batch',expectedBoundary:reason});assert.equal(validate(input),reason,id);});
const primitivePairs=[[4,0],[5,0],[6,0],[7,0],[38,17],[39,17],[41,17],[Number(MAX),17],[10,-1]];
assert.equal(v.primitiveSplitControls.length,9);
v.primitiveSplitControls.forEach((r,i)=>{const [n,e]=primitivePairs[i],p=split(n,e);assert.deepEqual(r,{totalBars:n,embargoBars:e,expectedStatus:p?'ok':'refused',expectedPlan:p});});
const sourcePaths=['alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs','alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs','alpha-factor-forge/src-tauri/src/research/trial_ledger.rs','alpha-factor-forge/src-tauri/src/research/trial_ledger_workspace.rs','alpha-factor-forge/src-tauri/src/research/mod.rs',...['split','walk_forward','embargo','identity','seed','enumerate','precision'].map(n=>'alpha-factor-forge/src-tauri/src/discovery_core/'+n+'.rs'),...['0001_trial_ledger.sql','0002_origin_genesis.sql','0003_family_protocol_upgrades.sql','0004_alpha_reservations.sql','0005_alpha_history_provenance.sql','0006_confirmation_history.sql','0007_confirmation_source_witness.sql'].map(n=>'alpha-factor-forge/src-tauri/registry_migrations/'+n)];
assert.deepEqual(v.sourcePins.map(p=>p.path),sourcePaths);
for(const p of v.sourcePins)assert.equal(p.lfSha256,sha(execFileSync('git',['show',baseline+':'+p.path],{cwd:root,encoding:'utf8'}).replace(/\r\n/g,'\n')));
const packages=['original-market-consumer-v2.json','original-market-source-projections-v1.json','original-market-registry-projections-v1.json','original-warmup-consumer-v1.json','original-producer-projections-v1.json','original-producer-mode-coverage-v1.json','original-warmup-producer-projections-v1.json','original-lineage-semantics-v1.json','source-row-graph-v1.json'];
assert.deepEqual(v.sourceFixtures.map(p=>p.path),packages);
for(const p of v.sourceFixtures)assert.equal(p.lfSha256,sha(fs.readFileSync(path.join(dir,p.path),'utf8').replace(/^\uFEFF/,'').replace(/\r\n/g,'\n')));
console.log('PASS independent Node:11cases/41candidates, original split/WF/null/seed/strategy/engine/event/family/member/batch projections,6ledger controls,24producer refusals,9split controls,19source pins/9immutable packages');
