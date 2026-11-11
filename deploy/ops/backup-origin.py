#!/usr/bin/env python3
"""Keep verified, root-only snapshots of the direct HTTPS entry point."""
import datetime as dt
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

os.umask(0o077)
root = Path('/root/zhiqiao-origin-backups')
root.mkdir(mode=0o700, exist_ok=True)
lock = (root / '.lock').open('a')
fcntl.flock(lock, fcntl.LOCK_EX)
stamp = dt.datetime.now(dt.timezone.utc).strftime('%Y%m%dT%H%M%S%fZ')
stage = root / ('.partial-' + stamp)
stage.mkdir(mode=0o700)
paths = ['etc/nginx', 'etc/letsencrypt', 'etc/letsencrypt-ip',
         'etc/ssh', 'etc/ufw', 'etc/fail2ban', 'etc/apt/apt.conf.d',
         'etc/apt/sources.list.d/zhiqiao-security.sources',
         'etc/systemd/system/zhiqiao-ip-cert-renew.service',
         'etc/systemd/system/zhiqiao-ip-cert-renew.timer',
         'etc/systemd/system/zhiqiao-origin-backup.service',
         'etc/systemd/system/zhiqiao-origin-backup.timer',
         'usr/local/sbin/zhiqiao-origin-backup.py']
archive = stage / 'origin.tar.gz'
subprocess.run(['tar', '-C', '/', '-czf', str(archive), *paths], check=True)
subprocess.run(['tar', '-tzf', str(archive)], check=True, stdout=subprocess.DEVNULL)
with archive.open('rb') as inp:
    digest = hashlib.file_digest(inp, 'sha256').hexdigest()
(stage / 'manifest.json').write_text(json.dumps({
    'created_utc': stamp, 'sha256': digest, 'bytes': archive.stat().st_size,
    'paths': paths}, indent=2) + '\n')
for item in stage.iterdir():
    with item.open('rb') as inp:
        os.fsync(inp.fileno())
stage.rename(root / stamp)
link = root / '.latest-new'
link.unlink(missing_ok=True)
link.symlink_to(stamp, target_is_directory=True)
link.replace(root / 'latest')
fd = os.open(root, os.O_RDONLY | os.O_DIRECTORY)
os.fsync(fd)
os.close(fd)
snapshots = sorted((p for p in root.iterdir() if not p.is_symlink()
                    and p.is_dir() and re.fullmatch(r'\d{8}T\d{12}Z', p.name)), reverse=True)
for old in snapshots[60:]:
    if old.parent.resolve() != root.resolve():
        raise RuntimeError('Unsafe retention path')
    shutil.rmtree(old)
print('Origin backup verified:', root / stamp)
