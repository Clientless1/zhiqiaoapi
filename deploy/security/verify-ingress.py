#!/usr/bin/env python3
"""Non-destructive ingress checks; run on origin to isolate the rate-limit key."""
import collections
import json
from pathlib import Path
import subprocess
import tempfile
import time

base = 'https://111.228.31.204'

def request(path, data=None):
    with tempfile.TemporaryDirectory() as tmp:
        headers = Path(tmp) / 'headers'
        cmd = ['curl', '--silent', '--show-error', '--max-time', '20',
               '--resolve', '111.228.31.204:443:127.0.0.1',
               '-D', str(headers), '-w', '\n%{http_code}', base + path]
        if data is not None:
            cmd += ['-H', 'Content-Type: application/json', '--data', data]
        result = subprocess.run(cmd, check=True, capture_output=True, text=True)
        body, code = result.stdout.rsplit('\n', 1)
        parsed = {}
        for line in headers.read_text().splitlines():
            if ':' in line:
                key, value = line.split(':', 1)
                parsed[key.lower()] = value.strip()
        return int(code), parsed, body

checks = {}
code, headers, _ = request('/')
assert code == 200
for name in ('x-content-type-options', 'x-frame-options', 'referrer-policy',
             'content-security-policy'):
    assert headers.get(name), name
assert headers.get('server') == 'nginx'
checks['headers_and_tls'] = 'passed'
for path in ('/.env', '/.git/config', '/%2eenv', '/api/.env'):
    code, _, _ = request(path)
    assert code == 404, (path, code)
checks['hidden_paths'] = '404'
for path in ('/api/me', '/api/admin/settings/service-policy',
             '/api/user/overview', '/v1/models'):
    code, _, _ = request(path)
    assert code in (401, 403), (path, code)
checks['unauthenticated_protected_routes'] = 'rejected'
code, _, body = request('/api/setup/status')
assert code == 200 and json.loads(body)['setup_completed'] is True
code, _, _ = request('/api/setup/providers')
assert code == 409
checks['setup_completed_guard'] = 'passed'

# Missing required fields cannot register a user or increment a real account's
# failed-login counter. Only loopback consumes this test's nginx limit budget.
codes = [request('/api/login', '{}')[0] for _ in range(16)]
assert 422 in codes and 429 in codes, codes
code, limited_headers, body = request('/api/login', '{}')
assert code == 429 and limited_headers.get('retry-after') == '60'
assert json.loads(body)['error']['code'] == 'authentication_rate_limited'
assert request('/healthz')[0] == 200
assert request('/v1/models')[0] == 401
checks['authentication_rate_limit'] = dict(collections.Counter(codes))
checks['api_excluded_from_auth_budget'] = 'passed'

probe = 'security-query-probe-' + str(time.time_ns())
assert request('/healthz?token=' + probe)[0] == 200
time.sleep(1)
lines = Path('/var/log/nginx/zhiqiao-direct.access.log').read_text().splitlines()[-100:]
parsed = [json.loads(line) for line in lines if line.startswith('{')]
assert any(line['uri'] == '/healthz' for line in parsed)
assert all(probe not in line for line in lines)
checks['outer_access_log_excludes_query'] = 'passed'
print(json.dumps(checks, indent=2))
