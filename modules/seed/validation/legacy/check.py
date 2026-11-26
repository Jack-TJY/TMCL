"""Re-run vanilla structure probes without starting a server or fetching game files."""
import argparse
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent
HASHES = {
    '1.12.2': '886945bfb2b978778c3a0288fd7fab09d315b25f',
    '1.18': '3cf24a8694aca6267883b17d934efacc5e44440d',
}


def verify(version, server, jdk, work):
    if hashlib.sha1(server.read_bytes()).hexdigest() != HASHES[version]:
        raise ValueError(f'Expected the official {version} server jar')
    java = str(jdk / 'bin/java'); javac = str(jdk / 'bin/javac')
    def run(*args):
        subprocess.run(args, cwd=work, timeout=180, check=True)
    if version == '1.12.2':
        cp = os.pathsep.join([str(work), str(server)])
        run(javac, '--release', '8', '-proc:none', '-cp', cp, '-d', str(work), str(ROOT/'Oracle12.java'))
        run(java, '-Xmx1400m', '-cp', cp, 'Oracle12')
    else:
        # Only unwrap embedded jars, under generated names in a temporary directory.
        jars = []
        with zipfile.ZipFile(server) as bundle:
            for name in sorted(bundle.namelist()):
                if name.endswith('.jar') and name.startswith(('META-INF/versions/', 'META-INF/libraries/')):
                    target = work / f'lib{len(jars)}.jar'
                    with bundle.open(name) as src, target.open('wb') as dst:
                        shutil.copyfileobj(src, dst)
                    jars.append(str(target))
        cp = os.pathsep.join(jars)
        run(javac, '--release', '17', '-proc:none', '-cp', cp, '-d', str(work), str(ROOT/'Oracle.java'))
        loader = work/'loader'; loader.mkdir()
        run(javac, '--release', '17', '-proc:none', '-d', str(loader), str(ROOT/'Run.java'))
        for seed in ('0', '-1'):
            run(java, '-Xmx1400m', '-XX:ActiveProcessorCount=4', '-cp', str(loader)+os.pathsep+cp, 'helper.Run', seed)
    import json
    baseline = json.loads((ROOT/'rings.json').read_text(encoding='utf-8'))[version]
    if set(baseline) != {'0','-1'}:
        raise RuntimeError('Incomplete ring fixture')
    for seed, expected in baseline.items():
        actual = (work/f'rings-{seed}.txt').read_text().splitlines()
        if len(actual) != 128 or len(set(actual)) != 128 or actual != expected:
            raise RuntimeError(f'{version}/{seed}: vanilla rings differ from fixture')
    print(f'{version}: both 128-position vanilla rings reproduced')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('version', choices=HASHES)
    parser.add_argument('server', type=Path)
    parser.add_argument('jdk', type=Path, help='JDK 17 or 18 home')
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix='tmcl-rings-') as directory:
        verify(args.version, args.server.resolve(), args.jdk.resolve(), Path(directory))
