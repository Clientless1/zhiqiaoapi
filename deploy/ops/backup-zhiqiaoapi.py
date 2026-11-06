#!/usr/bin/env python3
"""Back up PostgreSQL, application secrets/assets and deployment sources."""
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess as sp

os.umask(0o077)
ROOT = Path('/home/zhiqiao/backups/zhiqiaoapi')
APP = Path('/home/zhiqiao/zhiqiaoapi')
ROOT.mkdir(parents=True, exist_ok=True, mode=0o700)
lock = (ROOT / '.lock').open('a')
try:
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
except BlockingIOError:
    raise SystemExit('Another backup is running')
stamp = dt.datetime.now(dt.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
stage = ROOT / ('.partial-' + stamp)
stage.mkdir(mode=0o700)

def save(name, args):
    with (stage / name).open('xb') as out:
        sp.run(args, stdout=out, check=True, timeout=1800)
        out.flush()
        os.fsync(out.fileno())

try:
    save('database.dump', ['docker', 'exec', 'zhiqiaoapi-postgres-1', 'sh', '-ec',
        'exec pg_dump -U "$POSTGRES_USER" -d "$POSTGRES_DB" -Fc'])
    save('database-globals.sql', ['docker', 'exec', 'zhiqiaoapi-postgres-1', 'sh', '-ec',
        'exec pg_dumpall -U "$POSTGRES_USER" --globals-only'])
    save('backend-config.tar.gz', ['docker', 'exec', 'zhiqiaoapi-backend-1',
        'tar', '-C', '/app-config', '-czf', '-', '.'])
    save('backend-assets.tar.gz', ['docker', 'exec', 'zhiqiaoapi-backend-1',
        'tar', '-C', '/var/lib/zhiqiaoapi/assets', '-czf', '-', '.'])
    save('deployment.tar.gz', ['tar', '-C', str(APP), '--exclude=./.git',
        '--exclude=./build.log', '--exclude=node_modules', '--exclude=target',
        '--exclude=dist', '-czf', '-', '.'])
    with (stage / 'database.dump').open('rb') as inp:
        sp.run(['docker', 'exec', '-i', 'zhiqiaoapi-postgres-1', 'pg_restore', '--list'],
               stdin=inp, stdout=sp.DEVNULL, check=True, timeout=120)
    for name in ['backend-config.tar.gz', 'backend-assets.tar.gz', 'deployment.tar.gz']:
        sp.run(['tar', '-tzf', str(stage / name)], stdout=sp.DEVNULL, check=True)
    counts_sql = '''SELECT 'users', count(*) FROM "user"
        UNION ALL SELECT 'channels',count(*) FROM channel
        UNION ALL SELECT 'user_keys',count(*) FROM user_key
        UNION ALL SELECT 'usage',count(*) FROM usage
        UNION ALL SELECT 'billing',count(*) FROM billing
        UNION ALL SELECT 'settings',count(*) FROM setting;'''
    counts = sp.check_output(['docker', 'exec', 'zhiqiaoapi-postgres-1', 'sh', '-ec',
        'psql -At -U "$POSTGRES_USER" -d "$POSTGRES_DB" -c "$1"', 'sh', counts_sql], text=True)
    files = {f.name: {'bytes': f.stat().st_size, 'sha256': hashlib.file_digest(f.open('rb'), 'sha256').hexdigest()}
             for f in stage.iterdir() if f.is_file()}
    (stage / 'manifest.json').write_text(json.dumps({'created_utc': stamp,
        'database_counts': counts, 'files': files}, indent=2) + '\n')
    for f in stage.iterdir():
        with f.open('rb') as inp:
            os.fsync(inp.fileno())
    final = ROOT / stamp
    stage.rename(final)
    link = ROOT / '.latest-new'
    if link.is_symlink():
        link.unlink()
    link.symlink_to(stamp, target_is_directory=True)
    link.replace(ROOT / 'latest')
    fd = os.open(ROOT, os.O_RDONLY | os.O_DIRECTORY)
    os.fsync(fd)
    os.close(fd)
    # Keep 48 newest snapshots and one snapshot per day for the last 30 days.
    snapshots = sorted((p for p in ROOT.iterdir() if p.is_dir() and not p.is_symlink()
                        and re.fullmatch(r'\d{8}T\d{12}Z', p.name)), reverse=True)
    keep = {p.name for p in snapshots[:48]}
    daily = {}
    cutoff = (dt.datetime.now(dt.timezone.utc) - dt.timedelta(days=30)).strftime('%Y%m%d')
    for p in snapshots:
        if p.name[:8] >= cutoff:
            daily.setdefault(p.name[:8], p.name)
    keep.update(daily.values())
    for p in snapshots:
        if p.name not in keep:
            if p.parent.resolve() != ROOT.resolve() or p.is_symlink():
                raise RuntimeError('Unsafe retention target')
            shutil.rmtree(p)
    print('Backup verified:', final)
except BaseException:
    print('Backup failed; incomplete snapshot retained at', stage)
    raise
