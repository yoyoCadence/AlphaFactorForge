// Independent read-only source/DDL/recipe oracle. No backend or product DB calls.
const fs=require('node:fs'),path=require('node:path'),assert=require('node:assert/strict'),crypto=require('node:crypto');
const {execFileSync}=require('node:child_process');
const root=path.resolve(__dirname,'../..'),folder=path.join(root,'alpha-factor-forge/fixtures/research');
const baseline='394dd102e309492221c86d18f42af4728c284144',prefix='alpha-factor-forge/src-tauri/';
const sha=s=>crypto.createHash('sha256').update(s).digest('hex');
const cmp=(a,b)=>Buffer.compare(Buffer.from(a),Buffer.from(b));
const sorted=x=>Array.isArray(x)?x.map(sorted):x&&typeof x==='object'?Object.fromEntries(Object.keys(x).sort(cmp).map(k=>[k,sorted(x[k])])):x;
const json=x=>JSON.stringify(sorted(x));
const read=n=>JSON.parse(fs.readFileSync(path.join(folder,n),'utf8').replace(/^\uFEFF/,''));
const v=process.argv[2]?JSON.parse(fs.readFileSync(process.argv[2],'utf8')):read('original-trial-registry-capture-v1.json');
const parent=read('original-trial-projections-v1.json');
const original=p=>execFileSync('git',['show',baseline+':'+p],{cwd:root,encoding:'utf8'}).replace(/\r\n/g,'\n');
const keys=(x,names)=>assert.deepEqual(Object.keys(x).sort(),[...names].sort());
keys(v,['version','baseline','scope','sourcePins','sourceFixtures','knownMigrations','tables','totalColumns','fixedLimits','producerRecipes','boundaryRecipes','syntaxControls','requiredBehaviorControls','notAuthored']);
assert.equal(v.version,'original-trial-registry-capture-v1');assert.equal(v.baseline,baseline);
assert.equal(v.scope,'Unconsumed fixed capture contract only; SQL shape/bytes is not hash, execution, count, current binding, custody or admission authority.');
const versions=['0001_trial_ledger','0002_origin_genesis','0003_family_protocol_upgrades','0004_alpha_reservations','0005_alpha_history_provenance','0006_confirmation_history','0007_confirmation_source_witness'];
assert.deepEqual(v.knownMigrations,versions);
assert.deepEqual(v.fixedLimits,{schemaRecords:1024,schemaUtf8Bytes:1048576,schemaCellUtf8Bytes:4096,indexesPerTable:32,dataRecords:10000,textUtf8Bytes:4096,jsonUtf8Bytes:4194304,jsonNodes:65536,jsonValueDepth:64,decodedKeyUtf8Bytes:1024,retainedUtf8Bytes:33554432,aggregateJsonNodes:262144});
const names=['registry_migrations','registry_meta','trial_families','trial_batches','trial_events','batch_receipts','family_protocol_upgrades','origin_genesis','origin_checkpoints','registry_imports','family_conflicts','registry_conflicts'];
const orders=[['version'],['key'],['family_id'],['batch_id'],['seq','event_id'],['batch_id','receipt_registry_id'],['family_id','tests_per_trial'],['origin_registry_id'],['origin_registry_id','origin_seq'],['import_id'],['conflict_id'],['conflict_id']];
const ledgerSource=original(prefix+'src/research/trial_ledger.rs');
const begin=ledgerSource.indexOf('fn migrate('),finish=ledgerSource.indexOf('fn broken(');assert(begin>=0&&finish>begin);
const ddl=[ledgerSource.slice(begin,finish),...versions.map(n=>original(prefix+'registry_migrations/'+n+'.sql'))].join('\n').replace(/--[^\r\n]*/g,'');
function splitSql(body){
 const fields=[];let start=0,depth=0,quote=false;
 for(let i=0;i<body.length;i++){
  const c=body[i];if(c==="'"){if(quote&&body[i+1]==="'"){i++;continue;}quote=!quote;}
  if(quote)continue;
  if(c==='(')depth++;if(c===')')depth--;
  if(c===','&&depth===0){fields.push(body.slice(start,i).trim());start=i+1;}
 }
 fields.push(body.slice(start).trim());assert.equal(depth,0);assert.equal(quote,false);return fields;
}
function table(name,position){
 const re=new RegExp('CREATE TABLE(?: IF NOT EXISTS)? '+name+' \\(','g'),matches=[...ddl.matchAll(re)];assert.equal(matches.length,1,name);
 const from=matches[0].index+matches[0][0].length;let depth=1,quote=false,end=from;
 for(;end<ddl.length;end++){const c=ddl[end];if(c==="'"){if(quote&&ddl[end+1]==="'"){end++;continue;}quote=!quote;}if(quote)continue;if(c==='(')depth++;if(c===')'&&--depth===0)break;}
 assert.equal(depth,0);const parts=splitSql(ddl.slice(from,end));
 const pk=parts.find(p=>/^PRIMARY KEY\s*\(/.test(p));
 const composite=pk?pk.slice(pk.indexOf('(')+1,pk.lastIndexOf(')')).split(',').map(k=>k.trim()):[];
 const cols=[],unique=[];
 for(const field of parts){
  if(/^(PRIMARY|FOREIGN|UNIQUE|CHECK|CONSTRAINT)\b/.test(field))continue;
  const m=/^([a-z_][a-z0-9_]*)\s+(TEXT|INTEGER)\b([\s\S]*)$/.exec(field);assert(m,name+':'+field);
  const [,column,type,tail]=m;let policy='opaque_utf8_text';
  const qualified=name+'.'+column;
  if(['trial_families.family_key','trial_families.protocol_json','trial_batches.members_json','trial_events.payload_json','family_protocol_upgrades.protocol_json','family_conflicts.detail_json','registry_conflicts.detail_json'].includes(qualified))policy='json_syntax_only';
  if(type==='INTEGER'){
   if(column==='effective')policy='integer_zero_or_one';
   else if(column==='tests_per_trial')policy='integer_two_to_i64_max';
   else if(['import_id','conflict_id','seq','origin_seq'].includes(column))policy='integer_one_to_i64_max';
   else {assert(['head_seq','added_events','skipped_events','conflicts','family_effective_before','batch_effective_trials'].includes(column));policy='integer_zero_to_i64_max';}
  }
  if(qualified==='trial_events.kind')policy='six_original_trial_kinds';
  let defaultSql=null;
  if(/\bDEFAULT\b/.test(tail)){assert(/DEFAULT\s+\(datetime\('now'\)\)\s*$/.test(tail));defaultSql="datetime('now')";}
  const primaryKeyOrdinal=/\bPRIMARY KEY\b/.test(tail)?1:composite.indexOf(column)+1;
  if(/\bUNIQUE\b/.test(tail))unique.push([column]);
  cols.push({name:column,sqliteType:type,notNullMetadata:/\bNOT NULL\b/.test(tail)?1:0,defaultSql,primaryKeyOrdinal,hidden:0,
   nullableValue:['trial_families.protocol_json','trial_batches.family_id','trial_events.family_id'].includes(qualified),valuePolicy:policy});
 }
 return {name,tableFlags:{schema:'main',type:'table',columnCount:cols.length,withoutRowid:0,strict:0},columns:cols,additionalUniqueKeys:unique.sort(),orderBy:orders[position].map(column=>({column,comparison:cols.find(c=>c.name===column).sqliteType==='INTEGER'?'numeric':'utf8_binary'}))};
}
const expectedTables=names.map(table);assert.deepEqual(v.tables,expectedTables);assert.equal(v.totalColumns,56);assert.equal(expectedTables.reduce((n,t)=>n+t.columns.length,0),56);
const nodeCount=x=>1+(Array.isArray(x)?x.reduce((n,v)=>n+nodeCount(v),0):x&&typeof x==='object'?Object.values(x).reduce((n,v)=>n+nodeCount(v),0):0);
function doc(role,raw,dynamicExample=false){return {role,utf8Bytes:Buffer.byteLength(raw),nodes:nodeCount(JSON.parse(raw)),exampleSha256:sha(raw),dynamicExample};}
function docs(c){const b=c.expectedBatch,result=[...b.events].sort((a,b)=>a.freshSequence-b.freshSequence).map(e=>doc('trial_events.payload_json',e.payloadJson));result.push(doc('trial_batches.members_json',b.membersJson));if(b.familyId!==null)result.push(doc('trial_families.family_key',b.familyKeyJson),doc('trial_families.protocol_json',b.protocolJson));return result;}
function counts(c){return {...Object.fromEntries(names.map(n=>[n,0])),registry_migrations:7,registry_meta:2,trial_events:c.batchInput.events.length,trial_batches:1,batch_receipts:1,trial_families:Number(c.expectedBatch.familyId!==null)};}
const expected=[];
function add(id,refs,producerActions,expectedRecords,documents,auditAssertions){expected.push({id,sources:refs.map(([collection,c])=>({collection,id:c.id,batchInputSha256:sha(json(c.batchInput)),projectionSha256:sha(json(c.expectedBatch))})),producerActions,expectedRecords,totalRecords:Object.values(expectedRecords).reduce((a,b)=>a+b,0),originalJsonDocuments:documents,jsonBytes:documents.reduce((n,d)=>n+d.utf8Bytes,0),jsonNodes:documents.reduce((n,d)=>n+d.nodes,0),auditAssertions});}
for(const [collection,list] of [['cases',parent.cases],['batchControls',parent.batchControls]])for(const c of list)add('native-'+c.id,[[collection,c]],['fresh_register'],counts(c),docs(c),['all56cells_equal_actual_original_writer_SQL','dynamic_registry_ids_chain_values_and_audit_timestamps_preserved']);
const known=parent.cases[0],unrelated=parent.batchControls.find(c=>c.id==='diagnostic-kind'),other=parent.cases.find(c=>c.id==='v1-market-full');
add('known-protocol-upgrade',[['cases',known]],['fresh_register','replay_same_batch_tests_per_trial_3'],{...counts(known),family_protocol_upgrades:1},[...docs(known),doc('family_protocol_upgrades.protocol_json',json({correction:'holm',testsPerTrial:3}))],['event_batch_members_receipt_and_base_protocol_2_unchanged','one_upgrade_3_row','recorded_at_preserved']);
add('unrelated-mixed-kind-batch',[['cases',known],['batchControls',unrelated]],['fresh_register','register_unrelated_complete_mixed_kind_batch'],{...counts(known),trial_events:15,trial_batches:2,batch_receipts:2},[...docs(known),...docs(unrelated)],['every_unrelated_event_and_batch_retained','no_filter_or_current_accounting_claim']);
add('original-import',[['cases',known]],['source_fresh_register','source_original_trial_export','fresh_receiver_original_import'],{...counts(known),origin_genesis:1,origin_checkpoints:3,registry_imports:1},docs(known),['added3_skipped0_conflicts0','three_source_checkpoints_and_one_source_genesis','source_receipt_origin_preserved','file_sha256_is_actual_export_bytes_SHA256','no_receiver_receipt_invented']);
add('original-receipt-conflict',[['cases',known]],['original_import_recipe','rewrite_only_receipt_family_effective_before_99_reseal_body_SHA','original_import_conflicting_export'],{...counts(known),origin_genesis:1,origin_checkpoints:3,registry_imports:2,family_conflicts:1},[...docs(known),doc('family_conflicts.detail_json',json(['receipt '+known.expectedBatch.batchId+'/'+'a'.repeat(32)+' differs']),true)],['added0_skipped3_conflicts1','old_receipt_before0_not_overwritten','one_original_import_conflict_row','all_prior_checkpoints_genesis_and_events_retained']);
add('original-origin-divergence',[['cases',known],['cases',other]],['original_import_recipe','other_source_fresh_register_different_request','rewrite_export_header_registry_id_to_first_source_rebuild_event_chain_head_and_body_SHA','original_import_divergent_history'],{...counts(known),trial_events:6,trial_batches:2,batch_receipts:2,origin_genesis:1,origin_checkpoints:3,registry_imports:2,registry_conflicts:1},[...docs(known),...docs(other).filter(d=>!['trial_families.family_key','trial_families.protocol_json'].includes(d.role)),doc('registry_conflicts.detail_json',json({reason:'checkpoint_diverged_or_incomplete'}))],['added3_skipped0_conflicts1','one_original_registry_conflict_row','old_three_checkpoints_unchanged_no_fork_checkpoints_added','event_batch_receipt_origins_remain_original_other_source']);
assert.deepEqual(v.producerRecipes,expected);assert.equal(expected.length,22);assert(names.every(t=>expected.some(c=>c.expectedRecords[t]>0)));

// Numbers are skipped as tokens; only JSON string tokens are decoded. This
// deliberately never calls JSON.parse on a numeric literal or whole document.
function inspect(raw){
 assert(raw.length<=4194304);const text=new TextDecoder('utf-8',{fatal:true}).decode(raw);let i=0,nodes=0;
 const ws=()=>{while(/[ \t\r\n]/.test(text[i]??'!'))i++;};
 function string(){const start=i++;assert.equal(text[start],'"');let escaped=false;for(;i<text.length;i++){const c=text[i];if(escaped){escaped=false;continue;}if(c==='\\'){escaped=true;continue;}if(c==='"'){const s=JSON.parse(text.slice(start,++i));for(let p=0;p<s.length;p++){const v=s.charCodeAt(p);if(v>=0xd800&&v<=0xdbff){assert(p+1<s.length&&s.charCodeAt(p+1)>=0xdc00&&s.charCodeAt(p+1)<=0xdfff);p++;}else assert(!(v>=0xdc00&&v<=0xdfff));}return s;}}assert.fail('unterminated string');}
 function value(depth){ws();assert(depth<=64);assert(++nodes<=65536);const c=text[i];
  if(c==='"'){string();return;}
  if(c==='['){i++;ws();if(text[i]===']'){i++;return;}for(;;){value(depth+1);ws();if(text[i]===']'){i++;return;}assert.equal(text[i++],',');}}
  else if(c==='{'){i++;const seen=new Set();ws();if(text[i]==='}'){i++;return;}for(;;){ws();const key=string();assert(Buffer.byteLength(key)<=1024&&!seen.has(key));seen.add(key);ws();assert.equal(text[i++],':');value(depth+1);ws();if(text[i]==='}'){i++;return;}assert.equal(text[i++],',');}}
  else {const m=/^(?:null|true|false|-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?)/.exec(text.slice(i));assert(m,'invalid scalar');i+=m[0].length;}
 }
 value(1);ws();assert.equal(i,text.length);return nodes;
}
const boundaries=[];
for(const [boundary,limit,unit,recipe] of [['records',10000,'records','append_distinct_bounded_opaque_meta_rows_after_original_known_batch'],['retained_utf8',33554432,'utf8_bytes','append_opaque_meta_text_and_valid_json_detail_fillers_after_measuring_every_actual_dynamic_cell'],['aggregate_nodes',262144,'json_value_nodes','append_non_target_conflict_detail_arrays_under_all_single_document_limits']])for(const extra of [0,1])boundaries.push({id:boundary+(extra?'-plus-one':'-exact'),boundary,amount:limit+extra,unit,expected:extra?'refused':'captured_shape',recipe,otherBudgetsMustFit:true});
for(const [boundary,limit] of [['text_utf8',4096],['document_utf8',4194304],['document_nodes',65536],['value_depth',64],['decoded_key_utf8',1024]])for(const extra of [0,1]){
 const n=limit+extra;let text;
 if(boundary==='text_utf8')text='é'.repeat(limit/2)+(extra?'x':'');
 else if(boundary==='document_utf8')text='"'+'é'.repeat((limit-2)/2)+(extra?'x':'')+'"';
 else if(boundary==='document_nodes')text='['+Array(n-1).fill('null').join(',')+']';
 else if(boundary==='value_depth')text='['.repeat(n-1)+'null'+']'.repeat(n-1);
 else text='{"'+'é'.repeat(limit/2)+(extra?'x':'')+'":null}';
 const raw=Buffer.from(text);if(boundary!=='text_utf8'){let accepted=true;try{inspect(raw);}catch{accepted=false;}assert.equal(accepted,!extra);}
 boundaries.push({id:boundary+(extra?'-plus-one':'-exact'),boundary,amount:n,expected:extra?'refused':'captured_shape',rawUtf8Bytes:raw.length,rawSha256:sha(raw),otherBudgetsMustFit:true});
}
assert.deepEqual(v.boundaryRecipes,boundaries);
const syntax=[['opaque-number-tokens','[9223372036854775808,-9223372036854775809,1e999999,-1e-999999]','captured_shape',5],['decoded-duplicate-key','{"é":1,"\\u00e9":2}','refused',null],['distinct-not-normalized-keys','{"é":1,"e\\u0301":2}','captured_shape',3],['invalid-utf8',Buffer.from([123,34,120,34,58,34,255,34,125]),'refused',null],['invalid-unicode-surrogate','{"x":"\\ud800"}','refused',null],['valid-unicode-surrogate-pair','{"x":"\\ud83d\\ude00"}','captured_shape',2],['not-json-NaN','{"n":NaN}','refused',null],['sql-null-versus-json-null','null','captured_shape',1]];
assert.equal(v.syntaxControls.length,syntax.length);
syntax.forEach(([id,text,expected,nodes],i)=>{const raw=Buffer.isBuffer(text)?text:Buffer.from(text);assert.deepEqual(v.syntaxControls[i],{id,rawHex:raw.toString('hex'),expected,nodes});let actual=null;try{actual=inspect(raw);}catch{}assert.equal(actual,nodes,id);});
const paths=['research/trial_ledger.rs','research/trial_ledger_binding.rs','research/trial_ledger_transfer.rs','research/trial_ledger_confirmation/archive.rs','research/confirmation_inputs.rs','research/confirmation_inputs/original_json.rs','research/confirmation_inputs/source_rows.rs','research/confirmation_inputs/source_rows/graph.rs','research/confirmation_inputs/source_rows/graph/rows.rs','research/confirmation_inputs/source_rows/graph/original_lineage.rs','research/confirmation_inputs/source_rows/graph/original_lineage/market.rs','research/confirmation_inputs/source_rows/graph/original_lineage/market/measure.rs','research/confirmation_inputs/source_rows/graph/original_lineage/market_controls.rs'].map(p=>prefix+'src/'+p).concat(versions.map(n=>prefix+'registry_migrations/'+n+'.sql'));
assert.deepEqual(v.sourcePins.map(p=>p.path),paths);for(const p of v.sourcePins)assert.equal(p.lfSha256,sha(original(p.path)));
const packages=['original-trial-projections-v1.json','original-market-consumer-v2.json','source-row-graph-v1.json'];assert.deepEqual(v.sourceFixtures.map(p=>p.path),packages);for(const p of v.sourceFixtures)assert.equal(p.lfSha256,sha(fs.readFileSync(path.join(folder,p.path),'utf8').replace(/^\uFEFF/,'').replace(/\r\n/g,'\n')));
assert.deepEqual(v.requiredBehaviorControls,['actual_handle_only_no_foreign_connection_argument','main_qualified_queries_ignore_temp_or_attached_shadow_rows','already_active_registry_transaction_refused_without_ending_it','second_writer_growth_between_count_and_data_is_outside_pinned_snapshot','later_capture_sees_growth_and_refuses_over_capacity','all56_cells_raw_documents_nullable_values_and_complete_unique_order','all12_tables_populated_by_native_import_and_conflict_originals','opaque_malformed_hashes_not_semantically_verified','imported_and_mixed_kind_history_retained_not_filtered','i64_max_preserved_exactly_without_f64','extreme_number_tokens_preserved_without_numeric_conversion','decoded_duplicate_keys_and_invalid_utf8_or_unicode_refused','view_virtual_shadow_hidden_generated_and_extra_columns_refused','unknown_missing_duplicate_migrations_refused','blob_real_null_and_integer_domain_refusals','non_target_overflow_or_invalid_record_refuses_whole_capture','alpha_confirmation_nonempty_unread_and_unchanged','all_schema_triggers_pragmas_cells_real_bits_writer_total_changes_and_artifacts_unchanged','all_error_paths_release_mutex_and_preserve_caller_workspace_transaction_and_lease','copied_same_id_registry_yields_historical_shape_without_current_replacement_claim']);
assert.deepEqual(v.notAuthored,['actual_registry_ids_chain_values_file_hashes_and_audit_timestamps','full_coupled_market_trial_vectors','whole_consumer_joint_work_or_semantic_success','BindingCheck::Current','AdmissionCount','VerifiedDataset','RegisteredSource','current_owner_or_final_fence']);
console.log('PASS independent Node:12tables/56columns,22original producer recipes,16exact/+1 structural recipes,8opaque syntax controls,20historical source pins/3immutable packages');
