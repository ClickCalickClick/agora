#!/usr/bin/env python3
"""Scrub a raw response body into a committable fixture.
   scrub.py RAW_BODY OUT_FILE --user NAME [--keep N] [--redact-bodies]
Replaces every username with a stable pseudonym, drops tokens (modhash, csrf,
anything key-named like a token), trims listings to N children."""
import argparse, json, re, sys
ap = argparse.ArgumentParser()
ap.add_argument('raw'); ap.add_argument('out')
ap.add_argument('--user', required=True)
ap.add_argument('--keep', type=int, default=3)
ap.add_argument('--redact-bodies', action='store_true')
a = ap.parse_args()

names = {a.user.lower(): 'agora_tester'}
def pseudo(n):
    if not isinstance(n, str) or n in ('[deleted]', 'AutoModerator'):
        return n
    return names.setdefault(n.lower(), f'user{len(names)}')

SECRET = re.compile(r'(modhash|csrf|token|uh$|session|cookie|email)', re.I)
USERKEYS = {'author', 'dest', 'username', 'name_user', 'link_author', 'approved_by',
            'banned_by', 'mod_reason_by', 'author_flair_text'}
BODYKEYS = {'body', 'body_html', 'selftext', 'selftext_html', 'subject'}

def walk(o, parent_kind=None):
    if isinstance(o, dict):
        kind = o.get('kind')
        out = {}
        for k, v in o.items():
            if SECRET.search(k) and not isinstance(v, (dict, list, bool)) and v is not None:
                out[k] = 'REDACTED'
            elif k in USERKEYS:
                out[k] = pseudo(v)
            elif k == 'author_fullname' and isinstance(v, str):
                out[k] = 't2_' + pseudo(o.get('author', v))
            elif k == 'name' and parent_kind == 't2':
                out[k] = pseudo(v)
            elif a.redact_bodies and k in BODYKEYS and isinstance(v, str):
                out[k] = 'REDACTED'
            elif k == 'children' and isinstance(v, list) and v and isinstance(v[0], dict) and 'kind' in v[0]:
                out[k] = [walk(c) for c in v[:a.keep]]
            elif k == 'data':
                out[k] = walk(v, kind)
            else:
                out[k] = walk(v)
        return out
    if isinstance(o, list):
        return [walk(x) for x in o]
    if isinstance(o, str):
        return re.sub(re.escape(a.user), 'agora_tester', o, flags=re.I)
    return o

text = open(a.raw).read()
try:
    data = walk(json.loads(text))
    out = json.dumps(data, indent=1, ensure_ascii=False)
except json.JSONDecodeError:
    out = re.sub(re.escape(a.user), 'agora_tester', text, flags=re.I)
    out = re.sub(r'(csrf_token|modhash|token)(["\'=:\s]+)[A-Za-z0-9._-]{8,}', r'\1\2REDACTED', out, flags=re.I)
# Final guard: the account name must not survive anywhere.
assert a.user.lower() not in out.lower(), 'username survived scrubbing'
open(a.out, 'w').write(out)
print(a.out, len(out))
