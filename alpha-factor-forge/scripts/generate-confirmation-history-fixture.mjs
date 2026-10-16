// Independent Node encoding; no product encoder, Rust or provider invocation.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
function canonical(v) {
  const n = x => { const b = Buffer.alloc(4); b.writeUInt32BE(x); return b; };
  const s = x => { const b = Buffer.from(x); return Buffer.concat([n(b.length), b]); };
  if (v === null) return Buffer.from([0]);
  if (typeof v === 'boolean') return Buffer.from([v ? 2 : 1]);
  if (typeof v === 'number') { if (!Number.isSafeInteger(v)) throw Error('Unsafe fixture integer'); const b=Buffer.alloc(9);b[0]=3;b.writeDoubleBE(v,1);return b; }
  if (typeof v === 'string') return Buffer.concat([Buffer.from([4]),s(v)]);
  if (Array.isArray(v)) return Buffer.concat([Buffer.from([5]),n(v.length),...v.map(canonical)]);
  const keys=Object.keys(v).sort((a,b)=>Buffer.compare(Buffer.from(a),Buffer.from(b)));
  return Buffer.concat([Buffer.from([6]),n(keys.length),...keys.flatMap(k=>[s(k),canonical(v[k])])]);
}
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const record = body => ({ id: hash(Buffer.concat([Buffer.from(body.contractVersion+'\0'),canonical(body)])), body });
const base=JSON.parse(readFileSync(new URL('../fixtures/rs-core/research-confirmation-contracts-v1.json',import.meta.url),'utf8'));
const reservations={};const records={};
for(const [stage,batch,op] of [['validation',base.validation,base.validationOperation],['test',base.test,base.testOperation]]){
  const body={...base.alphaReservation.body,confirmationBatchId:batch.id,reservationKey:op.body.reservationKey,confirmationNumber:stage==='validation'?1:2};
  const r=record(body);reservations[stage]={reservationId:r.id,body};
  records[stage+'Exposure']=record({contractVersion:'research-confirmation-exposure-v1',originRegistryId:op.body.originRegistryId,
    reservationKey:op.body.reservationKey,instrumentId:batch.body.source.instrumentId,intervals:batch.body.split[stage+'Support'],
    claim:{kind:'confirmation',operationId:op.id,alphaReservationId:r.id}});
}
const common={contractVersion:'research-confirmation-exposure-v1',originRegistryId:'2'.repeat(32),reservationKey:'7'.repeat(32),
  instrumentId:base.validation.body.source.instrumentId,intervals:base.validation.body.split.validationSupport};
records.manual=record({...common,claim:{kind:'manual',accessKind:'raw',afterSealId:null}});
records.legacy=record({...common,reservationKey:'8'.repeat(32),claim:{kind:'legacy_unknown',reason:'migrated_workspace',sourceEvidenceSha256:null}});
const event=variant=>record({contractVersion:'research-confirmation-event-v1',operationId:base.validationOperation.id,admissionId:'9'.repeat(64),event:variant});
records.started=event({kind:'execution_started',workspaceId:base.validationOperation.body.workspaceId,ownershipEpoch:7,executionKey:'a'.repeat(32)});
records.sealed=event({kind:'sealed',executionStartedId:records.started.id,reportSha256:hash(base.reportJson),resultArtifactSha256:'b'.repeat(64),resultArtifactBytes:12345,engineManifestHash:base.validation.body.source.engineManifestHash});
records.interrupted=event({kind:'interrupted_spent',executionStartedId:records.started.id,reason:'owner_lost'});
records.failed=event({kind:'failed_spent',executionStartedId:null,reason:'input_missing'});
records.cancelled=event({kind:'cancelled_spent',executionStartedId:null,reason:'cancelled'});
for(const kind of ['origin_key','admission','exposure_overlap','manual_overlap','execution_started','seal','terminal']){
  records['conflict_'+kind]=record({contractVersion:'research-confirmation-conflict-v1',kind,claimIds:['c'.repeat(64),'d'.repeat(64)],
    affectedFamilyIds:[base.validation.body.source.familyId],affectedInstrumentIds:[base.validation.body.source.instrumentId]});
}
const fixture={description:'Synthetic single-record identities and supplied bindings only. Admission/conflict/seal references deliberately do not prove a complete graph, authenticity or execution authority.',reservations,records};
writeFileSync(new URL('../fixtures/rs-core/research-confirmation-history-records-v1.json',import.meta.url),JSON.stringify(fixture,null,2)+'\n');
console.log(JSON.stringify({records:Object.keys(records).length,validationExposure:records.validationExposure.id,started:records.started.id}));
