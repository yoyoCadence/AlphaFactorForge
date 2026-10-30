"""Author additive seed-v1 corrections using stdlib only; run from repo root.

Inputs are immutable original fixtures. Never import/execute product code or use
the unpublished market consumer as an oracle. Resource definitions are enumerated
BEFORE stored-row poisoning. The separate Node verifier rederives every value.
"""
from pathlib import Path
import copy
import hashlib
import itertools
import json
import re
import struct

ROOT = Path(__file__).resolve().parents[2]
FOLDER = ROOT / 'alpha-factor-forge/fixtures/research'


def read(name):
    return json.loads((FOLDER / name).read_text(encoding='utf-8-sig'))


def compact(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False)


def sha(value):
    return hashlib.sha256(value.encode() if isinstance(value, str) else value).hexdigest()


def u32(value):
    return struct.pack('>I', value)


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
    return b'\6' + u32(len(value)) + b''.join(
        text(key) + binary(value[key]) for key in sorted(value, key=lambda k: k.encode()))


def seed_frame(root_seed, dataset_hash, strategy_hash):
    return b'seed-v1\0' + u32(root_seed) + b''.join(
        text(value) for value in [dataset_hash, strategy_hash, 'random-entry'])


def vector(root_seed, old_dataset, new_dataset, strategy_hash):
    old_frame = seed_frame(root_seed, old_dataset, strategy_hash)
    frame = seed_frame(root_seed, new_dataset, strategy_hash)
    digest = hashlib.sha256(frame).digest()
    assert len(old_frame) == len(frame) == 195
    old_seed = struct.unpack('>I', hashlib.sha256(old_frame).digest()[:4])[0]
    seed = struct.unpack('>I', digest[:4])[0]
    return {'strategyHash': strategy_hash, 'priorRandomEntry': old_seed,
            'randomEntry': seed, 'preimageHex': frame.hex(), 'sha256': digest.hex(),
            'decimalWidthDelta': len(str(seed)) - len(str(old_seed))}


def enumerate_original(config):
    """Finite frozen grids only: last axis fastest; prune, first dedup, hash sort."""
    found = {}
    for base in config['bases']:
        axes = base['axes']
        grids = [range(a['min'], a['max'] + 1, a['step']) for a in axes]
        for values in itertools.product(*grids):
            definition = copy.deepcopy(base['strategy'])
            definition.update({a['key']: float(v) for a, v in zip(axes, values)})
            if definition['mode'] != 'dsl' and not all(
                    definition[a] < definition[b]
                    for a, b in [('fastMA', 'slowMA'), ('macdFast', 'macdSlow'), ('rsiBuy', 'rsiSell')]):
                continue
            payload = {'definition': definition, 'execModel': {
                'feePct': definition['feePct'], 'slippagePct': definition['slipPct']}}
            identity = 'strategy-v2:' + sha(b'strategy-v2\0' + binary(payload))
            found.setdefault(identity, definition)
    return sorted(found)


def adjust_preparation(preparation, delta):
    for key in ['parseBytes', 'scratchBytes']:
        preparation[key] += delta


def main():
    old = read('original-market-consumer-v1.json')
    out = copy.deepcopy(old)
    producer = read('original-producer-projections-v1.json')
    warm = read('original-warmup-consumer-v1.json')
    template = lambda identifier: copy.deepcopy(json.loads(next(
        c['configJson'] for c in producer['cases'] if c['id'] == identifier)))
    out['version'] = 'original-market-consumer-v2'
    out['baseline'] = '8d12f6ac7274328f45468b6a7ed2b75807f25426'
    correction = {
        'supersedes': {'path': 'original-market-consumer-v1.json',
                       'lfSha256': sha((FOLDER / 'original-market-consumer-v1.json').read_bytes().replace(b'\r\n', b'\n'))},
        'reason': 'v1 rebound dataset identity without rederiving seed-v1; original writers are unchanged',
        'originalSourcePinsBaseline': old['baseline'],
        'seedSourcePins': {p: sha((ROOT / p).read_bytes().replace(b'\r\n', b'\n')) for p in [
            'alpha-factor-forge/src-tauri/src/discovery_core/seed.rs',
            'alpha-factor-forge/src-tauri/src/discovery_core/identity.rs',
            'alpha-factor-forge/src-tauri/src/discovery_core/enumerate.rs']},
        'frame': 'UTF8 seed-v1 + NUL + rootSeed u32be + three u32be-length UTF8 datasetHash/strategyHash/random-entry fields; first four SHA256 bytes u32be',
        'familyVectors': [], 'resourceManifests': []}
    for case in out['cases']:
        family = case['family']
        base = next(c for c in warm['cases'] if c['id'] == family['sourceCase'])
        old_dataset = json.loads(base['configJson'])['dataset']['contentHash']
        root_seed = json.loads(family['configJson'])['rootSeed']
        identities = enumerate_original(json.loads(family['configJson']))
        vectors = []
        for original, item in zip(base['candidates'], family['inputs']):
            parsed = json.loads(item['inputFingerprintJson'])
            identity = parsed['strategyHash']
            assert identity == identities[item['index']]
            v = vector(root_seed, old_dataset, family['datasetHash'], identity)
            assert parsed['seeds']['randomEntry'] == v['priorRandomEntry']
            assert json.loads(original['inputFingerprintJson'])['seeds']['randomEntry'] == v['priorRandomEntry']
            # Replace exactly this numeric token; preserve original .0 and whitespace.
            item['inputFingerprintJson'], count = re.subn(
                r'("randomEntry":)' + str(v['priorRandomEntry']) + r'(?=\})',
                lambda m: m[1] + str(v['randomEntry']), item['inputFingerprintJson'])
            assert count == 1
            item['sha256'] = sha(item['inputFingerprintJson'])
            vectors.append(dict(index=item['index'], **v))
        delta = sum(v['decimalWidthDelta'] for v in vectors)
        adjust_preparation(family['priorPreparation'], delta)
        adjust_preparation(case['jointPreparation'], delta)
        correction['familyVectors'].append({'id': case['id'], 'priorDatasetHash': old_dataset,
            'datasetHash': family['datasetHash'], 'rootSeed': root_seed,
            'parseByteDelta': delta, 'candidates': vectors})

    for recipe in out['resourceBoundaryRecipes']:
        kind = recipe['kind']
        config = template('v1-ordered-axes' if kind in ['hash', 'output'] else 'v2-fixed-dsl')
        prior_dataset = config['dataset']['contentHash']
        config['dataset'] = {'id': 1, 'contentHash': out['cases'][0]['family']['datasetHash']}
        config['caps']['candidates'] = 4096
        base = config['bases'][0]
        if kind == 'hash':
            base['id'] = 'resource-main'
            base['axes'][0] = {'key': 'fastMA', 'min': 1, 'max': 8, 'step': 1}
        elif kind in ['analysis', 'copy']:
            reference = next(r for r in warm['resourceBoundaryRecipes'] if r['kind'] == kind)
            base['id'] = reference['baseId']
            base['strategy']['dsl']['name'] = reference['baseDslName']
            if kind == 'analysis':
                sidecar = copy.deepcopy(json.loads(warm['cases'][0]['configJson'])['bases'][0])
                sidecar['id'] = reference['paramsSidecar']['id']
                sidecar['axes'] = []
                config['bases'].append(sidecar)
        else:
            reference = warm['outputBoundaryRecipe']
            base['id'] = 'm' * recipe['mainIdBytes']
            base['strategy']['slowMA'] = reference['mainSlowMA']
            base['axes'] = reference['mainAxes']
            other = copy.deepcopy(base)
            other['id'] = 'o' * recipe['otherIdBytes']
            other['strategy']['emaPeriod'] = reference['otherEmaPeriod']
            other['axes'] = []
            config['bases'].append(other)
        vectors = [dict(index=i, **vector(config['rootSeed'], prior_dataset,
                   config['dataset']['contentHash'], identity))
                   for i, identity in enumerate(enumerate_original(config))]
        delta = sum(v['decimalWidthDelta'] for v in vectors)
        adjust_preparation(recipe['exactJointPreparation'], delta)
        adjust_preparation(recipe['overJointPreparation'], delta)
        lines = ''.join(f"{v['strategyHash']} {v['priorRandomEntry']} {v['randomEntry']}\n" for v in vectors)
        correction['resourceManifests'].append({'kind': kind,
            'meaning': 'original definitions/config BEFORE stored-row resource poison; exact and over share seeds',
            'originalConfigJson': compact(config), 'originalConfigHash': sha(compact(config)),
            'priorDatasetHash': prior_dataset, 'datasetHash': config['dataset']['contentHash'],
            'rootSeed': config['rootSeed'], 'candidateCount': len(vectors),
            'priorSeedDecimalBytes': sum(len(str(v['priorRandomEntry'])) for v in vectors),
            'seedDecimalBytes': sum(len(str(v['randomEntry'])) for v in vectors),
            'parseByteDelta': delta, 'sortedSeedLinesSha256': sha(lines), 'candidates': vectors})
    out['seedCorrection'] = correction
    target = FOLDER / 'original-market-consumer-v2.json'
    target.write_text(json.dumps(out, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')
    print(f'{target.name}: {target.stat().st_size} bytes SHA256 {sha(target.read_bytes())}')
    print('family deltas', [(r['id'], r['parseByteDelta']) for r in correction['familyVectors']])
    print('resource deltas', [(r['kind'], r['candidateCount'], r['parseByteDelta'], r['sortedSeedLinesSha256']) for r in correction['resourceManifests']])


if __name__ == '__main__':
    main()
