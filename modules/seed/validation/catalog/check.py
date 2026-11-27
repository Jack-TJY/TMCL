"""Offline vanilla locate catalog: export identifiers only, not generator parameters or assets."""
import argparse
from collections import defaultdict
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent
HASH = {'1.12.2': '886945bfb2b978778c3a0288fd7fab09d315b25f',
        '1.18': '3cf24a8694aca6267883b17d934efacc5e44440d',
        '26.3': '33680f5f2ac32864d6d7cf5e56a705fdb3e05f4c'}
DIMS = ('minecraft:overworld', 'minecraft:the_nether', 'minecraft:the_end')
# Semantic names for 1.12.2's unnamespaced commands, not additional /locate literals.
LEGACY = {'Temple': ['desert_pyramid','igloo','jungle_pyramid','swamp_hut'],
          'Mineshaft': ['mineshaft','mineshaft_mesa'], 'Village': ['village'],
          'Mansion': ['mansion'], 'Stronghold': ['stronghold'], 'Monument': ['monument'],
          'EndCity': ['end_city'], 'Fortress': ['fortress']}


def old_rows(version, lines):
    targets, assigned = set(), set()
    by_dim = {key: defaultdict(set) for key in DIMS}
    for line in lines:
        if 'CAT ' not in line:
            continue
        event, *args = line.split('CAT ', 1)[1].split()
        if event == 'target':
            name, = args
            targets.add(name)
        elif event == 'row':
            dim, name, variant = args
            assigned.add(name)
            members = LEGACY[name] if version == '1.12.2' else [variant]
            by_dim[dim][name].update(members)
        else:
            raise ValueError(f'unexpected probe record: {line}')
    if not targets or assigned != targets:
        raise ValueError(f'unaccounted locate targets: {targets ^ assigned}')
    if version == '1.12.2' and targets != LEGACY.keys():
        raise ValueError('legacy command vocabulary changed')
    return {'version': version, 'dimensions': {d: {k: sorted(v) for k,v in m.items()} for d,m in by_dim.items()}, 'tags': {}}


def expand_tags(tags, universe):
    """Resolve nested vanilla tags; never treat the tag name as a structure."""
    result = {}
    def visit(key, active):
        if key in result:
            return result[key]
        if key in active or key not in tags:
            raise ValueError(f'cyclic or unknown tag: {key}')
        members = set()
        for name in tags[key]:
            if name.startswith('#'):
                members.update(visit(name, active | {key}))
            elif name in universe:
                members.add(name)
            else:
                raise ValueError(f'{key}: missing registry entry {name}')
        result[key] = sorted(members)
        return result[key]
    for key in tags:
        visit(key, set())
    return result


def modern(jar, reports):
    """Intersect placed structures' biome selectors with normal-dimension biomes."""
    def records(folder):
        prefix = 'data/minecraft/' + folder + '/'
        return {'minecraft:'+n[len(prefix):-5]: json.loads(jar.read(n)) for n in jar.namelist()
                if n.startswith(prefix) and n.endswith('.json')}
    def groups(folder, ids):
        return expand_tags({'#'+k:v['values'] for k,v in records(folder).items()}, ids)
    commands = json.loads((reports/'commands.json').read_bytes())
    arg = commands['children']['locate']['children']['structure']['children']['structure']
    if not (arg['executable'] and arg['parser']=='minecraft:resource_or_tag_key' and arg['properties']['registry']=='minecraft:worldgen/structure'):
        raise ValueError('unexpected locate structure command')
    structures = records('worldgen/structure')
    placed = {v['structure'] for s in records('worldgen/structure_set').values() for v in s['structures'] if v['weight']>0}
    if placed != structures.keys():
        raise ValueError(f'registration != placement: {placed ^ structures.keys()}')
    biomes = records('worldgen/biome'); biome_tags = groups('tags/worldgen/biome', biomes)
    preset = records('worldgen/world_preset')['minecraft:normal']['dimensions']
    if set(preset)!=set(DIMS):
        raise ValueError('unexpected normal dimensions')
    by_dim = {}
    for dim in DIMS:
        source = preset[dim]['generator']['biome_source']
        if dim == 'minecraft:the_end' and source['type']=='minecraft:the_end':
            possible = set(biome_tags['#minecraft:is_end'])
        elif source['type']=='minecraft:multi_noise':
            name = source['preset'].removeprefix('minecraft:')
            report = reports/'biome_parameters'/'minecraft'/(name+'.json')
            possible = {row['biome'] for row in json.loads(report.read_bytes())['biomes']}
        else:
            raise ValueError(f'unrecognized biome source for {dim}')
        targets = {}
        for name, struct in structures.items():
            refs = struct['biomes']; refs = [refs] if isinstance(refs,str) else refs
            allowed = set()
            for ref in refs:
                if ref.startswith('#'):
                    allowed.update(biome_tags[ref])
                elif ref in biomes:
                    allowed.add(ref)
                else:
                    raise ValueError(f'unknown biome: {ref}')
            if possible & allowed:
                targets[name] = [name]
        by_dim[dim] = targets
    reachable = set().union(*by_dim.values())
    if reachable!=placed:
        raise ValueError(f'placed but unreachable: {placed-reachable}')
    return {'version':'26.3', 'dimensions':by_dim, 'tags':groups('tags/worldgen/structure',structures)}


def probe(version, server, jdk, work):
    def run(argv):
        proc = subprocess.run(argv, cwd=work, stdout=subprocess.PIPE,
                              stderr=subprocess.STDOUT, timeout=180)
        text = proc.stdout.decode('utf-8', errors='replace')
        if proc.returncode:
            raise RuntimeError(text[-3000:])
        return text.splitlines()

    javac = str(jdk / 'bin/javac')
    vm = [str(jdk / 'bin/java'), '-XX:ActiveProcessorCount=4', '-Xmx1500m']
    if version == '1.12.2':
        cp = os.pathsep.join([str(work), str(server)])
        run([javac, '--release', '8', '-proc:none', '-cp', cp, '-d', str(work),
             str(ROOT / 'Catalog12.java')])
        return old_rows(version, run(vm + ['-cp', cp, 'Catalog12']))

    with zipfile.ZipFile(server) as bundle:
        versions = [n for n in bundle.namelist()
                    if n.startswith('META-INF/versions/') and n.endswith('.jar')]
        if len(versions) != 1:
            raise ValueError('expected exactly one vanilla version jar')
        if version == '26.3':
            run(vm + ['-DbundlerMainClass=net.minecraft.data.Main', '-jar', str(server),
                      '--reports', '--output', 'generated'])
            with zipfile.ZipFile(io.BytesIO(bundle.read(versions[0]))) as inner:
                return modern(inner, work / 'generated/reports')

        libs = []
        library_names = sorted(n for n in bundle.namelist()
                               if n.startswith('META-INF/libraries/') and n.endswith('.jar'))
        for name in versions + library_names:
            dest = work / f'lib{len(libs)}.jar'
            libs.append(str(dest))
            with bundle.open(name) as src, dest.open('wb') as out:
                shutil.copyfileobj(src, out)
    cp = os.pathsep.join(libs)
    loader = work / 'loader'
    loader.mkdir()
    run([javac, '--release', '17', '-proc:none', '-cp', cp, '-d', str(work), str(ROOT / 'Oracle.java')])
    run([javac, '--release', '17', '-proc:none', '-d', str(loader), str(ROOT.parent / 'legacy/Run.java')])
    return old_rows(version, run(vm + ['-cp', str(loader) + os.pathsep + cp, 'helper.Run']))


def check_runtime(data, lines):
    """Compare a Runtime26 run without saving logs or printing unrelated messages."""
    expected = {
        ('RUNTIME', dim, name) for dim, names in data['dimensions'].items() for name in names
    } | {('GROUP', tag, name) for tag, names in data['tags'].items() for name in names}
    actual = set()
    for line in lines:
        for marker in ('RUNTIME minecraft:', 'GROUP #minecraft:'):
            pos = line.find(marker)
            if pos >= 0:
                actual.add(tuple(line[pos:].split()))
    if actual != expected:
        raise ValueError(f'live registry mismatch: missing={len(expected-actual)}, extra={len(actual-expected)}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('version', choices=HASH)
    parser.add_argument('server', type=Path)
    parser.add_argument('jdk', type=Path)
    parser.add_argument('--live-log', type=Path, help='also compare the output of Runtime26.sample')
    parser.add_argument('--output', type=Path, help='write derived metadata instead of comparing the checked-in catalog')
    args = parser.parse_args()
    if args.live_log and args.version != '26.3':
        parser.error('--live-log applies only to 26.3')
    server = args.server.resolve()
    if hashlib.sha1(server.read_bytes()).hexdigest() != HASH[args.version]:
        raise ValueError(f'expected the official {args.version} server jar')
    with tempfile.TemporaryDirectory(prefix='tmcl-catalog-') as tmp:
        data = probe(args.version, server, args.jdk.resolve(), Path(tmp))
    if args.live_log:
        with args.live_log.open(encoding='utf-8', errors='replace') as lines:
            check_runtime(data, lines)
    if args.output:
        args.output.write_text(json.dumps(data, sort_keys=True, indent=2)+'\n', encoding='utf-8')
    elif data != json.loads((ROOT / (args.version+'.json')).read_bytes()):
        raise ValueError('catalog does not match the local vanilla oracle')
    dims = data['dimensions']
    print(args.version, 'unique locate targets:', len(set().union(*dims.values())),
          {dim: len(dims[dim]) for dim in DIMS}, 'tags:', len(data['tags']))


if __name__ == '__main__':
    main()
