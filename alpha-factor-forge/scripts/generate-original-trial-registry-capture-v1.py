"""Finite capture-contract author. Only an ephemeral :memory: schema is opened.

No product import/registry/workspace/candle/source authority. Published v1 pins
its historical baseline even after future capture/parser visibility additions.
"""
from pathlib import Path
import copy
import hashlib
import json
import re
import sqlite3
import subprocess

ROOT = Path(__file__).resolve().parents[2]
FOLDER = ROOT / 'alpha-factor-forge/fixtures/research'
BASELINE = '394dd102e309492221c86d18f42af4728c284144'
SOURCE = 'alpha-factor-forge/src-tauri/'
VERSIONS = ['0001_trial_ledger','0002_origin_genesis','0003_family_protocol_upgrades',
            '0004_alpha_reservations','0005_alpha_history_provenance',
            '0006_confirmation_history','0007_confirmation_source_witness']
TABLES = ['registry_migrations','registry_meta','trial_families','trial_batches','trial_events',
          'batch_receipts','family_protocol_upgrades','origin_genesis','origin_checkpoints',
          'registry_imports','family_conflicts','registry_conflicts']
JSON_FIELDS = {'trial_families.family_key','trial_families.protocol_json','trial_batches.members_json',
               'trial_events.payload_json','family_protocol_upgrades.protocol_json',
               'family_conflicts.detail_json','registry_conflicts.detail_json'}
NULL_FIELDS = {'trial_families.protocol_json','trial_batches.family_id','trial_events.family_id'}
ORDERS = {
    'registry_migrations':['version'],'registry_meta':['key'],'trial_families':['family_id'],
    'trial_batches':['batch_id'],'trial_events':['seq','event_id'],
    'batch_receipts':['batch_id','receipt_registry_id'],
    'family_protocol_upgrades':['family_id','tests_per_trial'],'origin_genesis':['origin_registry_id'],
    'origin_checkpoints':['origin_registry_id','origin_seq'],'registry_imports':['import_id'],
    'family_conflicts':['conflict_id'],'registry_conflicts':['conflict_id'],
}


def compact(v):
    return json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=False,allow_nan=False)


def sha(v):
    return hashlib.sha256(v.encode('utf-8') if isinstance(v,str) else v).hexdigest()


def original(p):
    return subprocess.check_output(['git','show',BASELINE+':'+p]).decode('utf-8').replace('\r\n','\n')


def nodes(v):
    if isinstance(v,dict):
        return 1+sum(nodes(x) for x in v.values())
    if isinstance(v,list):
        return 1+sum(nodes(x) for x in v)
    return 1


def integer_domain(column):
    if column == 'effective':
        return 'integer_zero_or_one'
    if column == 'tests_per_trial':
        return 'integer_two_to_i64_max'
    if column in ['import_id','conflict_id','seq','origin_seq']:
        return 'integer_one_to_i64_max'
    assert column in ['head_seq','added_events','skipped_events','conflicts','family_effective_before','batch_effective_trials'],column
    return 'integer_zero_to_i64_max'


def schema():
    ledger = original(SOURCE+'src/research/trial_ledger.rs')
    start = ledger.index('CREATE TABLE IF NOT EXISTS registry_migrations (')
    opener = ledger[start:ledger.index(');',start)+2]
    # This private scratch DB has no path and no product data or backend calls.
    with sqlite3.connect(':memory:') as conn:
        conn.executescript(opener)
        for version in VERSIONS:
            conn.executescript(original(SOURCE+'registry_migrations/'+version+'.sql'))
        result=[]
        for table in TABLES:
            flags=next(r for r in conn.execute('PRAGMA table_list') if r[0]=='main' and r[1]==table)
            columns=[]
            for cid,name,kind,notnull,default,pk,hidden in conn.execute('PRAGMA table_xinfo('+table+')'):
                qualified=table+'.'+name
                policy = integer_domain(name) if kind=='INTEGER' else ('json_syntax_only' if qualified in JSON_FIELDS else 'opaque_utf8_text')
                if qualified=='trial_events.kind': policy='six_original_trial_kinds'
                columns.append({'name':name,'sqliteType':kind,'notNullMetadata':notnull,'defaultSql':default,
                                'primaryKeyOrdinal':pk,'hidden':hidden,'nullableValue':qualified in NULL_FIELDS,'valuePolicy':policy})
            by_name={c['name']:c for c in columns}
            unique=[]
            for _,name,is_unique,origin,_ in conn.execute('PRAGMA index_list('+table+')'):
                if is_unique and origin!='pk':
                    unique.append([r[2] for r in conn.execute('PRAGMA index_info('+name+')')])
            result.append({'name':table,'tableFlags':{'schema':flags[0],'type':flags[2],'columnCount':flags[3],'withoutRowid':flags[4],'strict':flags[5]},'columns':columns,'additionalUniqueKeys':sorted(unique),
                           'orderBy':[{'column':c,'comparison':'numeric' if by_name[c]['sqliteType']=='INTEGER' else 'utf8_binary'} for c in ORDERS[table]]})
        assert sum(len(t['columns']) for t in result)==56
        return result


def document(role,raw,dynamic=False):
    return {'role':role,'utf8Bytes':len(raw.encode('utf-8')),'nodes':nodes(json.loads(raw)),
            'exampleSha256':sha(raw),'dynamicExample':dynamic}


def projection(case):
    b=case['expectedBatch']
    documents=[]
    for event in sorted(b['events'],key=lambda e:e['freshSequence']):
        documents.append(document('trial_events.payload_json',event['payloadJson']))
    documents.append(document('trial_batches.members_json',b['membersJson']))
    if b['familyId'] is not None:
        documents.extend([document('trial_families.family_key',b['familyKeyJson']),document('trial_families.protocol_json',b['protocolJson'])])
    return documents


def fresh_counts(case):
    counts=dict.fromkeys(TABLES,0)
    counts.update(registry_migrations=7,registry_meta=2,trial_events=len(case['batchInput']['events']),
                  trial_batches=1,batch_receipts=1,trial_families=int(case['expectedBatch']['familyId'] is not None))
    return counts


def recipes(parent):
    native=[('cases',c) for c in parent['cases']]+[('batchControls',c) for c in parent['batchControls']]
    output=[]

    def add(identifier,refs,actions,counts,docs,audit):
        output.append({'id':identifier,'sources':[{'collection':collection,'id':c['id'],
            'batchInputSha256':sha(compact(c['batchInput'])),'projectionSha256':sha(compact(c['expectedBatch']))} for collection,c in refs],
            'producerActions':actions,'expectedRecords':counts,'totalRecords':sum(counts.values()),
            'originalJsonDocuments':docs,'jsonBytes':sum(d['utf8Bytes'] for d in docs),'jsonNodes':sum(d['nodes'] for d in docs),
            'auditAssertions':audit})

    for collection,c in native:
        add('native-'+c['id'],[(collection,c)],['fresh_register'],fresh_counts(c),projection(c),
            ['all56cells_equal_actual_original_writer_SQL','dynamic_registry_ids_chain_values_and_audit_timestamps_preserved'])
    known=parent['cases'][0]
    counts=fresh_counts(known);counts['family_protocol_upgrades']=1
    docs=projection(known)+[document('family_protocol_upgrades.protocol_json',compact({'correction':'holm','testsPerTrial':3}))]
    add('known-protocol-upgrade',[('cases',known)],['fresh_register','replay_same_batch_tests_per_trial_3'],counts,docs,
        ['event_batch_members_receipt_and_base_protocol_2_unchanged','one_upgrade_3_row','recorded_at_preserved'])
    unrelated=next(c for c in parent['batchControls'] if c['id']=='diagnostic-kind')
    counts=fresh_counts(known);counts['trial_events']+=12;counts['trial_batches']+=1;counts['batch_receipts']+=1
    add('unrelated-mixed-kind-batch',[('cases',known),('batchControls',unrelated)],['fresh_register','register_unrelated_complete_mixed_kind_batch'],counts,projection(known)+projection(unrelated),
        ['every_unrelated_event_and_batch_retained','no_filter_or_current_accounting_claim'])
    counts=fresh_counts(known);counts.update(origin_genesis=1,origin_checkpoints=3,registry_imports=1)
    add('original-import',[('cases',known)],['source_fresh_register','source_original_trial_export','fresh_receiver_original_import'],counts,projection(known),
        ['added3_skipped0_conflicts0','three_source_checkpoints_and_one_source_genesis','source_receipt_origin_preserved','file_sha256_is_actual_export_bytes_SHA256','no_receiver_receipt_invented'])
    counts=copy.deepcopy(counts);counts.update(registry_imports=2,family_conflicts=1)
    detail=compact(['receipt '+known['expectedBatch']['batchId']+'/'+'a'*32+' differs'])
    add('original-receipt-conflict',[('cases',known)],['original_import_recipe','rewrite_only_receipt_family_effective_before_99_reseal_body_SHA','original_import_conflicting_export'],counts,projection(known)+[document('family_conflicts.detail_json',detail,True)],
        ['added0_skipped3_conflicts1','old_receipt_before0_not_overwritten','one_original_import_conflict_row','all_prior_checkpoints_genesis_and_events_retained'])
    other=next(c for c in parent['cases'] if c['id']=='v1-market-full')
    counts=fresh_counts(known);counts.update(trial_events=6,trial_batches=2,batch_receipts=2,
        origin_genesis=1,origin_checkpoints=3,registry_imports=2,registry_conflicts=1)
    detail=compact({'reason':'checkpoint_diverged_or_incomplete'})
    # Both batches have one identical family; retain its key/base protocol once.
    docs=projection(known)+[d for d in projection(other) if d['role'] not in ['trial_families.family_key','trial_families.protocol_json']]+[document('registry_conflicts.detail_json',detail)]
    add('original-origin-divergence',[('cases',known),('cases',other)],['original_import_recipe','other_source_fresh_register_different_request','rewrite_export_header_registry_id_to_first_source_rebuild_event_chain_head_and_body_SHA','original_import_divergent_history'],counts,docs,
        ['added3_skipped0_conflicts1','one_original_registry_conflict_row','old_three_checkpoints_unchanged_no_fork_checkpoints_added','event_batch_receipt_origins_remain_original_other_source'])
    assert len(output)==22
    assert set().union(*(set(k for k,v in r['expectedRecords'].items() if v) for r in output))==set(TABLES)
    return output


def boundary_recipes():
    controls=[]
    for boundary,limit,unit in [('records',10000,'records'),('retained_utf8',33554432,'utf8_bytes'),('aggregate_nodes',262144,'json_value_nodes')]:
        for extra in [0,1]:
            controls.append({'id':boundary+('-exact' if extra==0 else '-plus-one'),'boundary':boundary,'amount':limit+extra,
                'unit':unit,'expected':'captured_shape' if extra==0 else 'refused','recipe':{
                    'records':'append_distinct_bounded_opaque_meta_rows_after_original_known_batch',
                    'retained_utf8':'append_opaque_meta_text_and_valid_json_detail_fillers_after_measuring_every_actual_dynamic_cell',
                    'aggregate_nodes':'append_non_target_conflict_detail_arrays_under_all_single_document_limits',
                }[boundary],'otherBudgetsMustFit':True})
    for boundary,limit in [('text_utf8',4096),('document_utf8',4194304),('document_nodes',65536),('value_depth',64),('decoded_key_utf8',1024)]:
        for extra in [0,1]:
            n=limit+extra
            if boundary=='text_utf8':
                value='\u00e9'*(limit//2)+('x' if extra else '')
                raw=value.encode('utf-8'); metric=len(raw)
            elif boundary=='document_utf8':
                value='"'+'\u00e9'*((limit-2)//2)+('x' if extra else '')+'"'
                raw=value.encode('utf-8');metric=len(raw)
            elif boundary=='document_nodes':
                raw=('['+','.join(['null']*(n-1))+']').encode();metric=n
            elif boundary=='value_depth':
                raw=('['*(n-1)+'null'+']'*(n-1)).encode();metric=n
            else:
                raw=('{"'+'\u00e9'*(limit//2)+('x' if extra else '')+'":null}').encode('utf-8');metric=n
            controls.append({'id':boundary+('-exact' if extra==0 else '-plus-one'),'boundary':boundary,'amount':metric,
                'expected':'captured_shape' if extra==0 else 'refused','rawUtf8Bytes':len(raw),'rawSha256':sha(raw),
                'otherBudgetsMustFit':True})
    return controls


def syntax_controls():
    escape=chr(92)
    examples=[
        ('opaque-number-tokens',b'[9223372036854775808,-9223372036854775809,1e999999,-1e-999999]',True,5),
        ('decoded-duplicate-key',('{"'+chr(233)+'":1,"'+escape+'u00e9":2}').encode('utf-8'),False,None),
        ('distinct-not-normalized-keys',('{"'+chr(233)+'":1,"e'+escape+'u0301":2}').encode('utf-8'),True,3),
        ('invalid-utf8',b'{"x":"'+bytes([255])+b'"}',False,None),
        ('invalid-unicode-surrogate',('{"x":"'+escape+'ud800"}').encode(),False,None),
        ('valid-unicode-surrogate-pair',('{"x":"'+escape+'ud83d'+escape+'ude00"}').encode(),True,2),
        ('not-json-NaN',b'{"n":NaN}',False,None),
        ('sql-null-versus-json-null',b'null',True,1),
    ]
    def refuse(*_):
        raise ValueError('invalid JSON syntax')
    def pairs(items):
        out={}
        for k,v in items:
            if k in out or len(k.encode('utf-8'))>1024:
                raise ValueError('invalid decoded key')
            out[k]=v
        return out
    def walk(v,depth=1):
        if depth>64:
            raise ValueError('value depth')
        if isinstance(v,str):
            v.encode('utf-8')
        if isinstance(v,dict):
            return 1+sum(walk(x,depth+1) for x in v.values())
        if isinstance(v,list):
            return 1+sum(walk(x,depth+1) for x in v)
        return 1
    result=[]
    for identifier,raw,expected,count in examples:
        try:
            value=json.loads(raw.decode('utf-8'),parse_int=str,parse_float=str,parse_constant=refuse,object_pairs_hook=pairs)
            actual=walk(value)
            accepted=True
        except (ValueError,UnicodeError):
            accepted=False;actual=None
        assert accepted==expected and actual==count,identifier
        result.append({'id':identifier,'rawHex':raw.hex(),'expected':'captured_shape' if expected else 'refused','nodes':count})
    return result


def main():
    parent=json.loads((FOLDER/'original-trial-projections-v1.json').read_text(encoding='utf-8-sig'))
    source_paths=[SOURCE+'src/'+p for p in ['research/trial_ledger.rs','research/trial_ledger_binding.rs',
        'research/trial_ledger_transfer.rs','research/trial_ledger_confirmation/archive.rs',
        'research/confirmation_inputs.rs','research/confirmation_inputs/original_json.rs',
        'research/confirmation_inputs/source_rows.rs','research/confirmation_inputs/source_rows/graph.rs',
        'research/confirmation_inputs/source_rows/graph/rows.rs',
        'research/confirmation_inputs/source_rows/graph/original_lineage.rs',
        'research/confirmation_inputs/source_rows/graph/original_lineage/market.rs',
        'research/confirmation_inputs/source_rows/graph/original_lineage/market/measure.rs',
        'research/confirmation_inputs/source_rows/graph/original_lineage/market_controls.rs']]
    source_paths += [SOURCE+'registry_migrations/'+v+'.sql' for v in VERSIONS]
    packages=['original-trial-projections-v1.json','original-market-consumer-v2.json','source-row-graph-v1.json']
    out={'version':'original-trial-registry-capture-v1','baseline':BASELINE,
        'scope':'Unconsumed fixed capture contract only; SQL shape/bytes is not hash, execution, count, current binding, custody or admission authority.',
        'sourcePins':[{'path':p,'lfSha256':sha(original(p))} for p in source_paths],
        'sourceFixtures':[{'path':p,'lfSha256':sha((FOLDER/p).read_text(encoding='utf-8-sig'))} for p in packages],
        'knownMigrations':VERSIONS,'tables':schema(),'totalColumns':56,
        'fixedLimits':{'schemaRecords':1024,'schemaUtf8Bytes':1048576,'schemaCellUtf8Bytes':4096,'indexesPerTable':32,
            'dataRecords':10000,'textUtf8Bytes':4096,'jsonUtf8Bytes':4194304,'jsonNodes':65536,'jsonValueDepth':64,
            'decodedKeyUtf8Bytes':1024,'retainedUtf8Bytes':33554432,'aggregateJsonNodes':262144},
        'producerRecipes':recipes(parent),'boundaryRecipes':boundary_recipes(),'syntaxControls':syntax_controls(),
        'requiredBehaviorControls':['actual_handle_only_no_foreign_connection_argument','main_qualified_queries_ignore_temp_or_attached_shadow_rows','already_active_registry_transaction_refused_without_ending_it',
            'second_writer_growth_between_count_and_data_is_outside_pinned_snapshot','later_capture_sees_growth_and_refuses_over_capacity',
            'all56_cells_raw_documents_nullable_values_and_complete_unique_order','all12_tables_populated_by_native_import_and_conflict_originals',
            'opaque_malformed_hashes_not_semantically_verified','imported_and_mixed_kind_history_retained_not_filtered',
            'i64_max_preserved_exactly_without_f64','extreme_number_tokens_preserved_without_numeric_conversion',
            'decoded_duplicate_keys_and_invalid_utf8_or_unicode_refused','view_virtual_shadow_hidden_generated_and_extra_columns_refused',
            'unknown_missing_duplicate_migrations_refused','blob_real_null_and_integer_domain_refusals',
            'non_target_overflow_or_invalid_record_refuses_whole_capture','alpha_confirmation_nonempty_unread_and_unchanged',
            'all_schema_triggers_pragmas_cells_real_bits_writer_total_changes_and_artifacts_unchanged',
            'all_error_paths_release_mutex_and_preserve_caller_workspace_transaction_and_lease',
            'copied_same_id_registry_yields_historical_shape_without_current_replacement_claim'],
        'notAuthored':['actual_registry_ids_chain_values_file_hashes_and_audit_timestamps','full_coupled_market_trial_vectors',
            'whole_consumer_joint_work_or_semantic_success','BindingCheck::Current','AdmissionCount','VerifiedDataset','RegisteredSource','current_owner_or_final_fence']}
    target=FOLDER/'original-trial-registry-capture-v1.json'
    target.write_text(json.dumps(out,ensure_ascii=False,indent=2,allow_nan=False)+'\n',encoding='utf-8',newline='\n')
    print(target.name,len(target.read_bytes()),'LF bytes SHA256',sha(target.read_bytes()))
    print('12 tables/56 columns;22 original producer recipes;16 structural exact/+1 recipes/8syntax controls;',len(source_paths),'historical source pins/3 immutable packages')


if __name__=='__main__':
    main()
