"""Offline woodland-mansion oracle for Java 1.18; 26.3 uses probe26.py."""
import argparse, hashlib, os, shutil, subprocess, tempfile, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
HASH = '3cf24a8694aca6267883b17d934efacc5e44440d'


def run(argv, cwd=None):
    proc = subprocess.run(argv, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                          timeout=180)
    if proc.returncode:
        raise RuntimeError(proc.stdout.decode('utf-8', errors='replace')[-3000:])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('server', type=Path)
    parser.add_argument('jdk', type=Path)
    args = parser.parse_args()
    server = args.server.resolve()
    if hashlib.sha1(server.read_bytes()).hexdigest() != HASH:
        raise ValueError('expected the official 1.18 server jar')
    javac = str(args.jdk / 'bin/javac'); java = str(args.jdk / 'bin/java')
    with tempfile.TemporaryDirectory(prefix='tmcl-mansion-') as tmp:
        work = Path(tmp)
        with zipfile.ZipFile(server) as bundle:
            names = [n for n in bundle.namelist() if n.endswith('.jar') and
                     n.startswith(('META-INF/versions/', 'META-INF/libraries/'))]
            libs = []
            for name in names:
                dest = work / f'lib{len(libs)}.jar'
                with bundle.open(name) as src, dest.open('wb') as out:
                    shutil.copyfileobj(src, out)
                libs.append(str(dest))
        cp = os.pathsep.join(libs)
        loader = work / 'loader'; loader.mkdir()
        run([javac, '--release', '17', '-proc:none', '-d', str(loader),
             str(ROOT.parent / 'legacy' / 'Run.java')], cwd=work)
        run([javac, '--release', '17', '-proc:none', '-cp', cp, '-d', str(work),
             str(ROOT / 'Mansion18.java')], cwd=work)
        proc = subprocess.run([java, '-XX:ActiveProcessorCount=4', '-Xmx1500m', '-cp',
                               str(loader) + os.pathsep + cp, 'helper.Run'],
                              cwd=work, stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                              timeout=180)
        if proc.returncode:
            raise RuntimeError(proc.stdout.decode('utf-8', errors='replace')[-3000:])
        for line in proc.stdout.decode('utf-8', errors='replace').splitlines():
            if 'MANSION ' in line:
                print(line[line.index('MANSION '):])


if __name__ == '__main__':
    main()
