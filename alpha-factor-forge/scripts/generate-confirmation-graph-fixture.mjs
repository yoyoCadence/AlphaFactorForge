// Independent byte encoder and integer arithmetic. No product implementation,
// bootstrap, reserved calibration seed or execution authority is invoked.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
function canonical(v) {
  const n=x=>{const b=Buffer.alloc(4);b.writeUInt32BE(x);return b;};
  const s=x=>{const b=Buffer.from(x);return Buffer.concat([n(b.length),b]);};
  if(v===null)return Buffer.from([0]);
  if(typeof v==='boolean')return Buffer.from([v?2:1]);
  if(typeof v==='number'){if(!Number.isSafeInteger(v))throw Error('unsafe integer');const b=Buffer.alloc(9);b[0]=3;b.writeDoubleBE(v,1);return b;}
  if(typeof v==='string')return Buffer.concat([Buffer.from([4]),s(v)]);
  if(Array.isArray(v))return Buffer.concat([Buffer.from([5]),n(v.length),...v.map(canonical)]);
  const keys=Object.keys(v).sort((a,b)=>Buffer.compare(Buffer.from(a),Buffer.from(b)));
  return Buffer.concat([Buffer.from([6]),n(keys.length),...keys.flatMap(k=>[s(k),canonical(v[k])])]);
}
const hash=x=>createHash('sha256').update(x).digest('hex');
const record=body=>({id:hash(Buffer.concat([Buffer.from(body.contractVersion+'\0'),canonical(body)])),body});
const base=JSON.parse(readFileSync(new URL('../fixtures/rs-core/research-confirmation-contracts-v1.json',import.meta.url),'utf8'));
const witnessed=process.argv.includes('--source-witness');
const sourceFixture=witnessed?JSON.parse(readFileSync(new URL('../fixtures/rs-core/research-source-witness-v1.json',import.meta.url),'utf8')):null;
if(witnessed){
  base.registeredSource=sourceFixture.registeredSource;
  base.validation=record({...base.validation.body,source:base.registeredSource.source});
  base.validationOperation=record({...base.validationOperation.body,confirmationBatchId:base.validation.id});
  base.selection.body.validationBatchId=base.validation.id;
  base.test.body.source=base.registeredSource.source;
}
const records={validation:base.validation,validationOperation:base.validationOperation};
const reservations={},blobs=[];
const origin=base.validationOperation.body.originRegistryId,family=base.registeredSource.source.familyId;
const chainHead=witnessed?sourceFixture.cases[0].anchor.chainHead:hash('trial-ledger-v1:genesis:'+origin);
const seq=witnessed?sourceFixture.cases[0].anchor.seq:0;
const binding={version:'trial-ledger-binding-v5',registryId:origin,seq,chainHead,
  evidence:{quarantinedFamilies:[],conflictedOrigins:[],familyTests:{[family]:2}},
  alpha:{budgets:[{familyId:family,allocationId:base.validation.body.statistics.allocationId}],reservations:[],conflicts:[]},
  alphaUnknownOrigins:[],confirmation:{batches:[],selections:[],operations:[],exposures:[],admissions:[],events:[],blobs:[],conflicts:[],unknownOrigins:[]}};
const bindingText=JSON.stringify(binding,null,1)+'\n';const bindingSha=hash(bindingText);
blobs.push({kind:'binding',sha256:bindingSha,text:bindingText});
if(witnessed)blobs.push({kind:'source_witness',sha256:sourceFixture.cases[0].witnessSha256,text:sourceFixture.cases[0].witnessText});
const ceil=(a,b)=>(a+b-1n)/b;
const m=8n,a=25000n,r=999999n;
const proof={contractVersion:'research-precision-v1',status:'ELIGIBLE',reasons:[],familyTrials:4,familyTests:8,
  minAttainableRawP:{numerator:1,denominator:800},bestAdjustedP:{numerator:8,denominator:800},
  requiredSamplesForResolution:Number(ceil(m*1000000n,a)-1n),
  requiredSamplesForPrecision:Number(ceil((1000000n*m-a)*1000000000000n,a*r*r))};
const plan={contractVersion:'research-precision-v1',correction:'holm',alphaPpm:25000,maxRelativeStandardErrorPpm:999999,
  priorTrials:2,plannedTrials:2,testsPerTrial:2,bootstrapSamples:799,maxBootstrapSamples:799};
const trial={registryId:origin,seq,chainHead,trialBatchId:'trial-batch-v1:'+base.registeredSource.source.trialBatchId,
  familyId:family,familyEffectiveTrials:4,registeredBatchEffectiveTrials:2,testsPerTrial:2,completeCandidates:2};
for(const stage of ['validation','test']){
  if(stage==='test'){
    records.selection=record({...base.selection.body,alphaReservationId:reservations.validation.reservationId,sealedReceiptId:records.validationSeal.id});
    records.test=record({...base.test.body,stage:{kind:'test',validationBatchId:records.validation.id,selectionId:records.selection.id}});
    records.testOperation=record({...base.testOperation.body,confirmationBatchId:records.test.id,selectionId:records.selection.id});
  }
  const batch=records[stage],op=records[stage+'Operation'];
  const alpha=record({...base.alphaReservation.body,confirmationBatchId:batch.id,reservationKey:op.body.reservationKey,confirmationNumber:stage==='validation'?1:2});
  reservations[stage]={reservationId:alpha.id,body:alpha.body};
  const exposure=record({contractVersion:'research-confirmation-exposure-v1',originRegistryId:origin,reservationKey:op.body.reservationKey,
    instrumentId:batch.body.source.instrumentId,intervals:batch.body.split[stage+'Support'],claim:{kind:'confirmation',operationId:op.id,alphaReservationId:alpha.id}});
  records[stage+'Exposure']=exposure;
  const admission=record({contractVersion:'research-confirmation-admission-v1',operationId:op.id,alphaReservationId:alpha.id,exposureId:exposure.id,
    workspaceObservation:{workspaceId:op.body.workspaceId,ownershipEpoch:7,mutationSeq:12,referencesSha256:'e'.repeat(64)},
    registryObservation:{registryId:origin,seq,chainHead,bindingSha256:bindingSha,familyEffectiveTrials:4,registeredBatchEffectiveTrials:2,testsPerTrial:2,
      protocolSha256:hash(JSON.stringify({correction:'holm',testsPerTrial:2}))},precisionPlan:plan,precisionProof:proof});
  records[stage+'Admission']=admission;
  const event=event=>record({contractVersion:'research-confirmation-event-v1',operationId:op.id,admissionId:admission.id,event});
  records[stage+'Start']=event({kind:'execution_started',workspaceId:op.body.workspaceId,ownershipEpoch:7,executionKey:(stage==='validation'?'a':'c').repeat(32)});
  const report=JSON.parse(base.reportJson);if(stage==='test'){report.seed=8;report.tests=report.tests.slice(0,2);}
  // Original whitespace and decimal presentation are deliberate hash evidence.
  const text=JSON.stringify(report,null,1).replaceAll('"observedMean": 0.1','"observedMean": 1e-1')+'\n';
  const sha=hash(text);blobs.push({kind:'statistics',sha256:sha,text});
  records[stage+'Seal']=event({kind:'sealed',executionStartedId:records[stage+'Start'].id,reportSha256:sha,resultArtifactSha256:'b'.repeat(64),resultArtifactBytes:12345,engineManifestHash:batch.body.source.engineManifestHash});
  if(stage==='validation')base.selection.body.reportSha256=sha;
}
const fixture={description:'Independent full archival Validation/selection/Test chain. Synthetic m=8 structural evidence only; no calibration, authentic host snapshot or execution rights.',
  registeredSource:base.registeredSource,allocationDeclaration:base.allocationDeclaration,trial,reservations,records,blobs};
if(witnessed)fixture.description='Independent strict source-witness Validation/selection/Test graph with an actual four-event source chain. Synthetic m=8 structure only; no statistical support or execution authority.';
const filename=witnessed?'research-confirmation-witness-graph-v1.json':'research-confirmation-graph-v1.json';
writeFileSync(new URL('../fixtures/rs-core/'+filename,import.meta.url),JSON.stringify(fixture,null,2)+'\n');
console.log(JSON.stringify({records:Object.keys(records).length,blobs:blobs.length,admission:records.validationAdmission.id}));
