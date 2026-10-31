"""Independent stdlib author for original trial projection fixtures.

Only declared pure-function arguments are modeled. This never imports/runs
product code, opens a registry/workspace, constructs VerifiedDataset or produces
an admission/receipt/current-count capability. Published v1 is immutable.
"""
from pathlib import Path
import copy
import hashlib
import json
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[2]
FOLDER = ROOT / 'alpha-factor-forge/fixtures/research'
BASELINE = 'bbf669af8f4cda80e63b7e12da42831e32fe8d75'
MAX = 9_007_199_254_740_991


def read(name):
    return json.loads((FOLDER / name).read_text(encoding='utf-8-sig'))


def compact(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False, allow_nan=False)


def sha(value):
    return hashlib.sha256(value.encode() if isinstance(value, str) else value).hexdigest()


def u32(number):
    return struct.pack('>I', number)


def text(value):
    encoded = value.encode()
    return u32(len(encoded)) + encoded


def binary(value):
    if value is None:
        return b'\0'
    if isinstance(value, bool):
        return b'\2' if value else b'\1'
    if isinstance(value, (int, float)):
        return b'\3' + struct.pack('>d', value if value else 0.)
    if isinstance(value, str):
        return b'\4' + text(value)
    if isinstance(value, list):
        return b'\5' + u32(len(value)) + b''.join(map(binary, value))
    return b'\6' + u32(len(value)) + b''.join(text(k) + binary(value[k]) for k in sorted(value, key=lambda k: k.encode()))


def strategy_hash(definition):
    payload = {'definition': definition, 'execModel': {'feePct': definition['feePct'], 'slippagePct': definition['slipPct']}}
    return 'strategy-v2:' + sha(b'strategy-v2\0' + binary(payload))


def seed(root_seed, dataset, strategy):
    frame = b'seed-v1\0' + u32(root_seed) + text(dataset) + text(strategy) + text('random-entry')
    digest = hashlib.sha256(frame).digest()
    assert len(frame) == 195
    return {'randomEntry': int.from_bytes(digest[:4], 'big')}, frame.hex(), digest.hex()


def bar_range(start, count):
    return {'from': start, 'to': start + count - 1, 'count': count}


def split_plan(total, embargo):
    if not (0 <= total <= MAX and 0 <= embargo <= MAX):
        return None
    usable = total - 2 * embargo
    if usable < 5:
        return None
    q, r = divmod(usable, 5)
    weights = [3, 1, 1]
    counts = [w * q + w * r // 5 for w in weights]
    # Sort by descending remainder, retaining Train/Validation/Test tie order.
    order = sorted(range(3), key=lambda i: (-(weights[i] * r % 5), i))
    for i in order[:usable - sum(counts)]:
        counts[i] += 1
    train = bar_range(0, counts[0])
    gap1 = bar_range(counts[0], embargo) if embargo else None
    validation = bar_range(counts[0] + embargo, counts[1])
    gap2 = bar_range(validation['to'] + 1, embargo) if embargo else None
    test = bar_range(validation['to'] + embargo + 1, counts[2])
    assert test['to'] == total - 1
    return {'totalBars': total, 'usableBars': usable, 'embargoBars': embargo, 'train': train,
            'trainValidationEmbargo': gap1, 'validation': validation, 'validationTestEmbargo': gap2, 'test': test}


def walk_report(total, embargo, declaration):
    plan = {'totalBars': total, 'embargoBars': embargo, **declaration}
    if any(not (0 <= n <= MAX) for n in plan.values()) or declaration['minimumTrainBars'] == 0 or declaration['foldValidationBars'] == 0 or not 2 <= declaration['foldCount'] <= 128:
        return None
    required = declaration['minimumTrainBars'] + declaration['foldCount'] * (embargo + declaration['foldValidationBars'])
    report = {'contractVersion': 'research-walk-forward-v1', 'plan': plan, 'status': 'NOT_ELIGIBLE',
              'reasons': [], 'outerTrain': None, 'availableTrainBars': None,
              'requiredTrainBars': required if required <= MAX else None, 'folds': []}
    outer = split_plan(total, embargo)
    if outer is None:
        report['reasons'] = ['insufficient_outer_history']
        return report
    available = outer['train']['count']
    report.update(outerTrain=outer['train'], availableTrainBars=available)
    if available < required:
        report['reasons'] = ['insufficient_train_history']
        return report
    cursor = available - declaration['foldCount'] * (embargo + declaration['foldValidationBars'])
    for index in range(declaration['foldCount']):
        train = bar_range(0, cursor)
        gap = bar_range(cursor, embargo) if embargo else None
        cursor += embargo
        validation = bar_range(cursor, declaration['foldValidationBars'])
        cursor = validation['to'] + 1
        report['folds'].append({'index': index, 'train': train, 'embargo': gap, 'validation': validation})
    assert cursor == available
    report['status'] = 'ELIGIBLE'
    return report


def identity(config, derivation, total):
    declaration = config.get('walkForward')
    wf = None if declaration is None else {'declaration': declaration, 'report': None if derivation is None else walk_report(total, derivation['embargoBars'], declaration)}
    return {'contracts': {'split': config['contracts']['split'], 'embargo': config['contracts']['embargo']},
            'totalBars': total, 'holdingAllowanceBars': config['embargo']['holdingAllowanceBars'],
            'embargo': derivation, 'split': None if derivation is None else split_plan(total, derivation['embargoBars']), 'walkForward': wf}


HASH_FIELDS = ['hypothesisHash', 'strategyHash', 'datasetHash', 'snapshotId', 'splitHash', 'seedsHash',
               'engineFingerprintHash', 'benchmarkId', 'benchmarkParamsHash', 'reproductionOf']


def validate_batch(batch):
    """Finite Variant/Hypothesis/Diagnostic/legacy-refusal catalog, not a new API."""
    if not batch['events']:
        return 'empty_events'
    workspace = batch['workspaceId']
    if not workspace.strip() or ':' in workspace:
        return 'workspace'
    if not 1 <= batch['testsPerTrial'] <= MAX:
        return 'protocol_count'
    instrument = batch['instrumentId']
    if instrument is not None and instrument not in ['crypto:binance:BTCUSDT', 'us-etf:nyse-arca:SPY', 'tw-etf:twse:0050']:
        return 'instrument'
    first_legacy = batch['events'][0]['origin']['type'] == 'legacy'
    requests, keys = set(), set()
    for event in batch['events']:
        for field in HASH_FIELDS:
            if event[field] is not None and not event[field].strip():
                return 'blank_' + field
        if (event['snapshotId'] is not None) != (instrument is not None):
            return 'snapshot_presence'
        origin = event['origin']
        if origin['type'] == 'request':
            if first_legacy or event['kind'] == 'legacy':
                return 'origin_kind'
            request = origin['requestId']
            if not request.strip() or ':' in request:
                return 'request'
            requests.add(request)
            key = f"{workspace}:request:{request}:candidate:{origin['candidateIndex']}"
        else:
            if not first_legacy or event['kind'] != 'legacy':
                return 'origin_kind'
            if not origin['attemptKey'].strip():
                return 'attempt_key'
            key = workspace + ':' + origin['attemptKey']
        if len(requests) > 1:
            return 'mixed_request'
        if key in keys:
            return 'duplicate_key'
        keys.add(key)
        assert event['kind'] in ['variant', 'hypothesis', 'diagnostic']
        for field in ['strategyHash', 'datasetHash', 'splitHash', 'seedsHash', 'engineFingerprintHash']:
            if event[field] is None:
                return 'missing_' + field
        for field in ['benchmarkId', 'benchmarkParamsHash', 'reproductionOf']:
            if event[field] is not None:
                return 'forbidden_' + field
    return None


def project_batch(batch):
    assert validate_batch(batch) is None
    instrument = batch['instrumentId']
    family_json = None if instrument is None else compact({'version': 'trial-family-v1', 'instrumentId': instrument})
    family = None if family_json is None else 'trial-family-v1:' + sha(family_json)
    events = []
    for position, event in enumerate(batch['events']):
        origin = event['origin']
        assert origin['type'] == 'request'
        key = f"{batch['workspaceId']}:request:{origin['requestId']}:candidate:{origin['candidateIndex']}"
        payload = {'version': 'trial-event-v1', 'familyId': family, 'kind': event['kind'], 'effective': True,
                   'idempotencyKey': key, 'workspaceId': batch['workspaceId'], 'requestId': origin['requestId'],
                   'candidateIndex': origin['candidateIndex'], 'legacyAttemptKey': None,
                   **{field: event[field] for field in HASH_FIELDS}}
        raw = compact(payload)
        assert len(payload) == 19
        events.append({'inputPosition': position, 'candidateIndex': origin['candidateIndex'],
                       'idempotencyKey': key, 'payloadJson': raw, 'eventId': 'trial-event-v1:' + sha(raw)})
    ordered = sorted(events, key=lambda e: e['idempotencyKey'].encode())
    members = [[e['idempotencyKey'], e['eventId']] for e in ordered]
    batch_json = compact({'version': 'trial-batch-v1', 'familyId': family, 'members': members})
    batch_id = 'trial-batch-v1:' + sha(batch_json)
    for sequence, event in enumerate(ordered, 1):
        event['freshSequence'] = sequence
        event['storedProjection'] = {'event_id': event['eventId'], 'family_id': family,
            'idempotency_key': event['idempotencyKey'], 'kind': json.loads(event['payloadJson'])['kind'],
            'effective': 1, 'batch_id': batch_id, 'payload_json': event['payloadJson']}
    return {'familyKeyJson': family_json, 'familyId': family, 'protocolJson': compact({'correction': 'holm', 'testsPerTrial': batch['testsPerTrial']}),
            'batchIdentityJson': batch_json, 'batchId': batch_id, 'membersJson': compact(members),
            'candidateOrderByMembers': [e['candidateIndex'] for e in ordered],
            'returnedEventIdsInputOrder': [e['eventId'] for e in events], 'events': events}


def main():
    market = read('original-market-consumer-v2.json')
    warm = read('original-warmup-consumer-v1.json')
    producers = read('original-producer-projections-v1.json')
    named = lambda values, identifier: next(v for v in values if v['id'] == identifier)

    def source(kind, identifier):
        if kind == 'market':
            case = named(market['cases'], identifier)
            f = case['family']
            old = named(warm['cases'], f['sourceCase'])
            candidates = []
            for saved, expected, derivation in zip(old['candidates'], f['expectedLineage']['candidates'], f['expectedWarmup']):
                candidates.append({'definition': json.loads(saved['freshStrategy']['original_definition_json']),
                    'strategyHash': expected['strategyHash'], 'hypothesisHash': expected['hypothesisHash'],
                    'expectedSeed': json.loads(f['inputs'][expected['index']]['inputFingerprintJson'])['seeds'], 'embargo': derivation})
            instrument = json.loads(f['campaignDocumentJson'])['instruments'][0]['instrumentId']
            return f['configJson'], old['engineFingerprintJson'], candidates, instrument, case['expectedMarket']['snapshotId']
        if kind == 'overflow':
            case = warm['derivationRefusalCases'][0]
            derivations = [v.get('derivation') for v in case['expectedPerCandidate']]
        else:
            case = named(producers['cases'], identifier)
            derivations = next(v['expectedWarmup'] for v in warm['existingFamilies'] if v['caseId'] == identifier and v['sourceFixture'] == 'original-producer-projections-v1.json')
        candidates = []
        observations = case['expectedObservation']['candidates'] if kind == 'overflow' else case['candidates']
        for saved, expected, derivation in zip(case['candidates'], observations, derivations):
            candidates.append({'definition': json.loads(saved['freshStrategy']['original_definition_json']),
                'strategyHash': expected['strategyHash'], 'hypothesisHash': expected['hypothesisHash'],
                'expectedSeed': json.loads(saved['inputFingerprintJson'])['seeds'], 'embargo': derivation})
        return case['configJson'], case['engineFingerprintJson'], candidates, None, None

    recipes = [
        ('v1-market-small', 'market', 'captured-old-revision', 4),
        ('v1-market-full', 'market', 'captured-old-revision', 600),
        ('v1-shared-dedup', 'producer', 'v1-shared-dedup', 600),
        ('v2-fixed-dsl', 'producer', 'v2-fixed-dsl', 600),
        ('v3-mixed-lookback-distinction', 'market', 'new-revision-mixed', 600),
        ('v3-mixed-insufficient-outer', 'market', 'new-revision-mixed', 4),
        ('v3-dsl-insufficient-train', 'producer', 'v3-walk-forward', 100),
        ('v3-mixed-null-embargo', 'overflow', 'v3-mixed-failed-embargo-overflow', 600),
        ('v1-twelve-lexical', 'grid', 'v1-ordered-axes', 600),
        ('etf-declared-family', 'market', 'etf-declared-references', 600),
        ('v3-outside-safe-count', 'producer', 'v3-walk-forward', MAX + 1),
    ]
    cases = []
    for identifier, kind, source_id, total in recipes:
        raw, engine, candidates, instrument, snapshot = source('producer' if kind == 'grid' else kind, source_id)
        config = json.loads(raw)
        if kind == 'grid':
            original = copy.deepcopy(config)
            config['bases'][0]['axes'][0] = {'key': 'fastMA', 'min': 1, 'max': 12, 'step': 1}
            assert config['bases'][0]['strategy'] == original['bases'][0]['strategy']
            raw = compact(config)
            base = config['bases'][0]
            definitions = []
            for fast in range(1, 13):
                definition = copy.deepcopy(base['strategy'])
                definition.update(fastMA=float(fast), slowMA=21.0)
                assert definition['entrySig'] == 'maCrossUp' and definition['exitSig'] == 'maCrossDown'
                definitions.append(definition)
            candidates = [{'definition': d, 'strategyHash': strategy_hash(d), 'hypothesisHash': candidates[0]['hypothesisHash'],
                'expectedSeed': None, 'embargo': {'maxSignalLookbackBars': 22, 'holdingAllowanceBars': 10, 'embargoBars': 32}}
                for d in sorted(definitions, key=strategy_hash)]
        config_hash = sha(compact(config))
        assert compact(json.loads(engine)) == engine
        details, events = [], []
        for index, candidate in enumerate(candidates):
            assert strategy_hash(candidate['definition']) == candidate['strategyHash']
            seeds, preimage, digest = seed(config['rootSeed'], config['dataset']['contentHash'], candidate['strategyHash'])
            if candidate['expectedSeed'] is not None:
                assert seeds == candidate['expectedSeed']
            document = identity(config, candidate['embargo'], total)
            wrapped = compact({'version': 'trial-split-v1', 'identity': document})
            split_hash = 'trial-split-v1:' + sha(wrapped)
            wf = config.get('walkForward')
            actual_report = {'status': 'not_applicable'}
            if wf is not None:
                if candidate['embargo'] is None:
                    actual_report = {'status': 'refused', 'phase': 'embargo'}
                elif wf['minimumTrainBars'] < candidate['embargo']['maxSignalLookbackBars']:
                    actual_report = {'status': 'refused', 'phase': 'candidate_lookback'}
                elif document['walkForward']['report'] is None:
                    actual_report = {'status': 'refused', 'phase': 'walk_forward_plan'}
                else:
                    actual_report = {'status': 'ok', 'report': document['walkForward']['report']}
            details.append({'index': index, 'strategyHash': candidate['strategyHash'], 'hypothesisHash': candidate['hypothesisHash'],
                'definitionJson': compact(candidate['definition']), 'seedsJson': compact(seeds), 'seedPreimageHex': preimage,
                'seedSha256': digest, 'seedsHash': sha(compact(seeds)), 'engineFingerprintHash': sha(engine),
                'identityJson': compact(document), 'splitIdentityPayloadJson': wrapped, 'splitHash': split_hash,
                'candidateWalkForwardReport': actual_report})
            events.append({'kind': 'variant', 'origin': {'type': 'request', 'requestId': 'request-' + identifier, 'candidateIndex': index},
                'hypothesisHash': candidate['hypothesisHash'], 'strategyHash': candidate['strategyHash'],
                'datasetHash': config['dataset']['contentHash'], 'snapshotId': snapshot, 'splitHash': split_hash,
                'seedsHash': sha(compact(seeds)), 'engineFingerprintHash': sha(engine),
                'benchmarkId': None, 'benchmarkParamsHash': None, 'reproductionOf': None})
        batch = {'workspaceId': 'trial-projection-workspace', 'instrumentId': instrument, 'testsPerTrial': 2, 'events': events}
        cases.append({'id': identifier, 'sourceKind': kind, 'sourceCase': source_id,
            'countAuthority': 'declared pure-function argument only; no VerifiedDataset/current-count evidence',
            'declaredCandleCount': total, 'configJson': raw, 'configHash': config_hash, 'configRawSha256': sha(raw),
            'engineFingerprintJson': engine, 'candidates': details, 'batchInput': batch, 'expectedBatch': project_batch(batch)})

    batch_controls = []
    grid = named(cases, 'v1-twelve-lexical')
    for identifier in ['reversed-input', 'raw-spaces', 'unversioned-split', 'absent-hypothesis', 'diagnostic-kind', 'protocol-three']:
        batch = copy.deepcopy(grid['batchInput'])
        if identifier == 'reversed-input':
            batch['events'].reverse()
        elif identifier == 'raw-spaces':
            batch['workspaceId'] = ' workspace-\u00e9 '
            for event in batch['events']:
                event['origin']['requestId'] = ' request-\u00e9 '
        elif identifier == 'unversioned-split':
            batch['events'][0]['splitHash'] = 'unversioned-original-claim'
        elif identifier == 'absent-hypothesis':
            batch['events'][0]['hypothesisHash'] = None
        elif identifier == 'diagnostic-kind':
            batch['events'][0]['kind'] = 'diagnostic'
        else:
            batch['testsPerTrial'] = 3
        batch_controls.append({'id': identifier, 'baseCase': grid['id'], 'ledgerPreparation': 'accepted',
            'meaning': 'original ledger projection only; no source suitability or registry replay/current-count assertion',
            'batchInput': batch, 'expectedBatch': project_batch(batch)})

    refusals = []
    def refusal(identifier, mutate, reason):
        batch = copy.deepcopy(cases[0]['batchInput']); mutate(batch)
        assert validate_batch(batch) == reason, (identifier, validate_batch(batch), reason)
        refusals.append({'id': identifier, 'batchInput': batch, 'expectedCode': 'invalid_batch', 'expectedBoundary': reason})
    refusal('empty-events', lambda b: b.update(events=[]), 'empty_events')
    refusal('empty-workspace', lambda b: b.update(workspaceId=' '), 'workspace')
    refusal('workspace-colon', lambda b: b.update(workspaceId='a:b'), 'workspace')
    refusal('zero-protocol', lambda b: b.update(testsPerTrial=0), 'protocol_count')
    refusal('protocol-over-safe', lambda b: b.update(testsPerTrial=MAX+1), 'protocol_count')
    refusal('bad-instrument', lambda b: b.update(instrumentId='bad'), 'instrument')
    refusal('snapshot-without-instrument', lambda b: b.update(instrumentId=None), 'snapshot_presence')
    refusal('missing-snapshot', lambda b: b['events'][0].update(snapshotId=None), 'snapshot_presence')
    refusal('empty-request', lambda b: b['events'][0]['origin'].update(requestId=' '), 'request')
    refusal('request-colon', lambda b: b['events'][0]['origin'].update(requestId='a:b'), 'request')
    refusal('mixed-request', lambda b: b['events'][1]['origin'].update(requestId='another'), 'mixed_request')
    refusal('duplicate-index-key', lambda b: b['events'][1]['origin'].update(candidateIndex=0), 'duplicate_key')
    refusal('legacy-kind-request-origin', lambda b: b['events'][0].update(kind='legacy'), 'origin_kind')
    refusal('legacy-origin-variant-kind', lambda b: b['events'][0].update(origin={'type':'legacy','attemptKey':'old'}), 'origin_kind')
    refusal('blank-hypothesis', lambda b: b['events'][0].update(hypothesisHash=' '), 'blank_hypothesisHash')
    for field in ['strategyHash', 'datasetHash', 'splitHash', 'seedsHash', 'engineFingerprintHash']:
        refusal('missing-' + field, lambda b, f=field: b['events'][0].update({f:None}), 'missing_' + field)
    for field in ['benchmarkId', 'benchmarkParamsHash', 'reproductionOf']:
        refusal('forbidden-' + field, lambda b, f=field: b['events'][0].update({f:'claimed'}), 'forbidden_' + field)
    refusal('blank-strategy', lambda b: b['events'][0].update(strategyHash=' '), 'blank_strategyHash')

    split_controls = []
    for total, embargo in [(4,0),(5,0),(6,0),(7,0),(38,17),(39,17),(41,17),(MAX,17),(10,-1)]:
        plan = split_plan(total, embargo)
        split_controls.append({'totalBars': total, 'embargoBars': embargo, 'expectedStatus': 'ok' if plan else 'refused', 'expectedPlan': plan})

    source_files = [
        'alpha-factor-forge/src-tauri/src/discovery_runner/execution.rs',
        'alpha-factor-forge/src-tauri/src/discovery_runner/mod.rs',
        'alpha-factor-forge/src-tauri/src/research/trial_ledger.rs',
        'alpha-factor-forge/src-tauri/src/research/trial_ledger_workspace.rs',
        'alpha-factor-forge/src-tauri/src/research/mod.rs',
        *['alpha-factor-forge/src-tauri/src/discovery_core/'+n+'.rs' for n in ['split','walk_forward','embargo','identity','seed','enumerate','precision']],
        *['alpha-factor-forge/src-tauri/registry_migrations/'+n for n in ['0001_trial_ledger.sql','0002_origin_genesis.sql','0003_family_protocol_upgrades.sql','0004_alpha_reservations.sql','0005_alpha_history_provenance.sql','0006_confirmation_history.sql','0007_confirmation_source_witness.sql']],
    ]
    source_pins = [{'path': p, 'lfSha256': sha(subprocess.check_output(['git','show',BASELINE+':'+p]).replace(b'\r\n',b'\n'))} for p in source_files]
    packages = ['original-market-consumer-v2.json','original-market-source-projections-v1.json','original-market-registry-projections-v1.json','original-warmup-consumer-v1.json','original-producer-projections-v1.json','original-producer-mode-coverage-v1.json','original-warmup-producer-projections-v1.json','original-lineage-semantics-v1.json','source-row-graph-v1.json']
    out = {'version': 'original-trial-projections-v1', 'baseline': BASELINE,
        'scope': 'Independent pure original-format projections only. Declared counts and hashes do not authenticate datasets, execution, original registry custody, receipts, current counts or source admission.',
        'platform': 'project64-bit target; outside-safe-count is an exact2^53 pure scalar, never allocated as candles',
        'inputCarrier': 'Fixture-only camelCase carrier for typed Rust inputs; no new runtime JSON API and no serializable BenchmarkEvidence',
        'sourcePins': source_pins, 'sourceFixtures': [{'path':p,'lfSha256':sha((FOLDER/p).read_text(encoding='utf-8-sig'))} for p in packages],
        'cases': cases, 'batchControls': batch_controls, 'producerValidationRefusals': refusals,
        'primitiveSplitControls': split_controls,
        'dynamicRegistryFieldsNotAuthored': ['registry_id','chain','origin_registry_id','registered_at','created_at','receipt_registry_id','admission/count/current-head/alpha/source-witness observations'],
        'futureCompleteMembershipRefusals': ['missing referenced event','duplicate candidate or idempotency key','extra batch event','selected subset matching only workspace attempts','wrong workspace/instrument/family/kind/origin/index/hypothesis/strategy/dataset/snapshot/split/seeds/engine','reordered or substituted members/event mapping','payload versus scalar projection contradiction','unsupported benchmark/reproduction/legacy proof promoted to a normal Variant','current protocol inferred from identity or old family protocol_json','declared candle_count substituted for authenticated dataset count','old/copied registry observation substituted for a current final fence'],
    }
    target=FOLDER/'original-trial-projections-v1.json'
    target.write_text(json.dumps(out,indent=2,ensure_ascii=False,allow_nan=False)+'\n',encoding='utf-8',newline='\n')
    print(target.name,target.stat().st_size,'bytes SHA256',sha(target.read_bytes()))
    print('cases/candidates',len(cases),sum(len(c['candidates']) for c in cases),'batch controls',len(batch_controls),'producer refusals',len(refusals),'split controls',len(split_controls))


if __name__ == '__main__':
    main()
