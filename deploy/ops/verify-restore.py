#!/usr/bin/env python3
"""Restore the latest dump in an isolated, temporary database container."""
import hashlib
import json
from pathlib import Path
import subprocess as sp
import time

backup = Path('/home/zhiqiao/backups/zhiqiaoapi/latest').resolve()
m = json.loads((backup / 'manifest.json').read_text())
for name, details in m['files'].items():
    with (backup / name).open('rb') as inp:
        assert hashlib.file_digest(inp, 'sha256').hexdigest() == details['sha256'], name
name = 'zhiqiaoapi-restore-verification'
image = sp.check_output(['docker', 'inspect', 'zhiqiaoapi-postgres-1',
                         '--format', '{{.Config.Image}}'], text=True).strip()
sp.run(['docker', 'run', '-d', '--name', name, '--network', 'none',
        '--tmpfs', '/var/lib/postgresql/data:rw,size=256m',
        '-e', 'POSTGRES_HOST_AUTH_METHOD=trust', '-e', 'POSTGRES_DB=restorecheck', image],
        check=True, stdout=sp.DEVNULL)
try:
    for _ in range(60):
        r = sp.run(['docker', 'exec', name, 'pg_isready', '-U', 'postgres', '-d', 'restorecheck'],
                   stdout=sp.DEVNULL, stderr=sp.DEVNULL)
        if r.returncode == 0:
            break
        time.sleep(0.5)
    else:
        raise RuntimeError('Temporary database failed to start')
    with (backup / 'database.dump').open('rb') as inp:
        sp.run(['docker', 'exec', '-i', name, 'pg_restore', '-U', 'postgres',
                '-d', 'restorecheck', '--no-owner', '--no-privileges', '--exit-on-error'],
               stdin=inp, check=True, timeout=180)
    q = '''SELECT 'users', count(*) FROM "user"
        UNION ALL SELECT 'channels',count(*) FROM channel
        UNION ALL SELECT 'user_keys',count(*) FROM user_key
        UNION ALL SELECT 'usage',count(*) FROM usage
        UNION ALL SELECT 'billing',count(*) FROM billing
        UNION ALL SELECT 'settings',count(*) FROM setting;'''
    counts = sp.check_output(['docker', 'exec', name, 'psql', '-At', '-U', 'postgres',
                             '-d', 'restorecheck', '-c', q], text=True)
    assert counts == m['database_counts'], 'Restored row counts differ from snapshot'
    print('PASS: full database restored successfully in an isolated temporary container')
    print(counts)
finally:
    sp.run(['docker', 'rm', '-f', name], check=True, stdout=sp.DEVNULL)
