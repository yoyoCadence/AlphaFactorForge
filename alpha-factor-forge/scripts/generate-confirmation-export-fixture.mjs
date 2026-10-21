// Independent wire oracle over an actual complete four-event source fixture.
// No product imports, simulation, input authentication or execution authority.
import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync} from 'node:fs';
const canonical=v=>Array.isArray(v)?`[${v.map(canonical).join(',')}]`:
  v!==null&&typeof v==='object'?`{${Object.keys(v).sort().map(k=>`${JSON.stringify(k)}:${canonical(v[k])}`).join(',')}}`:JSON.stringify(v);
const hash=text=>createHash('sha256').update(text,'utf8').digest('hex');
const original=JSON.parse(readFileSync(new URL('../fixtures/rs-core/research-source-witness-v1.json',import.meta.url),'utf8')).cases[0];
const witness=JSON.parse(original.witnessText);
const trial=witness.trialExportText;
const source=JSON.parse(trial.split('\n')[0]).registryId;
const exportedAt='2026-10-21T00:00:00Z';
const alphaBody=canonical({type:'trialLedger',export:trial})+'\n';
const alpha=canonical({version:'trial-ledger-export-v4',registryId:source,
  schemaVersion:'0005_alpha_history_provenance',exportedAt,budgetCount:0,
  reservationCount:0,conflictCount:0,unknownOriginCount:0,bodySha256:hash(alphaBody)})+'\n'+alphaBody;
const empty={batches:[],selections:[],operations:[],exposures:[],admissions:[],events:[],blobs:[],conflicts:[],unknownOrigins:[]};
const cases=[];
for(const populated of [false,true]){
 const inventory=structuredClone(empty);
 if(populated){inventory.blobs=[{kind:'source_witness',sha256:hash(original.witnessText),text:original.witnessText}];inventory.unknownOrigins=['e'.repeat(32)];}
 const counts=Object.fromEntries(Object.entries(inventory).map(([key,values])=>[key,values.length]));
 const rows=[{type:'alphaLedger',export:alpha},...inventory.blobs.map(b=>({type:'confirmationBlob',...b})),...inventory.unknownOrigins.map(originRegistryId=>({type:'confirmationUnknownOrigin',originRegistryId}))];
 const body=rows.map(canonical).join('\n')+'\n';
 const text=canonical({version:'trial-ledger-export-v5',registryId:source,
  schemaVersion:'0007_confirmation_source_witness',exportedAt,counts,bodySha256:hash(body)})+'\n'+body;
 cases.push({name:populated?'standalone-proof-and-unknown':'trial-only-history',registryId:source,alphaText:alpha,exportedAt,inventory,text,sha256:hash(text)});
}
writeFileSync(new URL('../fixtures/rs-core/research-confirmation-export-v5.json',import.meta.url),JSON.stringify({description:'Independent canonical export framing only; source is a complete original four-event snapshot, no execution rights.',cases},null,2)+'\n');
