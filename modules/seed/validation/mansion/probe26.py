"""Ask a local vanilla 26.3 dedicated server for the nearest woodland mansion (two seeds)."""
import json, os, re, shutil, subprocess, sys, tempfile
from pathlib import Path

SERVER = Path(sys.argv[1]).resolve()
JAVA = str(Path(sys.argv[2]) / 'bin/java.exe')


def run_seed(seed):
    with tempfile.TemporaryDirectory(prefix=f'tmcl-mansion-{seed}-') as tmp:
        tmp = Path(tmp)
        shutil.copyfile(SERVER, tmp/'server.jar')
        (tmp/'eula.txt').write_text('eula=true\n')
        (tmp/'server.properties').write_text(
            'level-seed=%d\nlevel-name=world\nonline-mode=false\n'
            'spawn-protection=0\nmax-players=1\nview-distance=2\n' % seed)
        proc = subprocess.Popen([JAVA, '-Xms512M', '-Xmx1500M', '-jar', 'server.jar', 'nogui'],
                                cwd=tmp, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT, text=True, encoding='utf-8',
                                errors='replace')
        out = []
        try:
            for line in proc.stdout:
                out.append(line)
                if 'Done (' in line:
                    proc.stdin.write('locate structure minecraft:mansion\n')
                    proc.stdin.flush()
                    proc.stdin.write('stop\n')
                    proc.stdin.flush()
        finally:
            proc.wait(timeout=180)
        text = ''.join(out)
    match = re.search(r'nearest minecraft:mansion is at \[(-?\d+),\s*~?[^,]*,\s*(-?\d+)\]', text)
    if not match:
        raise SystemExit(text[-2500:])
    print('MANSION valid %d %d %d' % (seed, int(match.group(1)) // 16, int(match.group(2)) // 16))


for seed in (0, -1):
    run_seed(seed)
