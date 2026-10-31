#!/usr/bin/env python3
"""Send one command to spike.py and print its result. Usage:
   ctl.py '{"cmd":"op","op":"get","args":{"url":"/api/me.json"}}'"""
import json, os, sys, time, uuid
BASE = os.path.expanduser('~/.local/share/agora-spike')
cmd = json.loads(sys.argv[1])
cmd['rid'] = cmd.get('rid') or time.strftime('%H%M%S-') + uuid.uuid4().hex[:6]
with open(os.path.join(BASE, 'ctl.fifo'), 'w') as f:
    f.write(json.dumps(cmd) + '\n')
path = os.path.join(BASE, 'out', cmd['rid'] + '.json')
for _ in range(1200):
    if os.path.exists(path):
        print(open(path).read())
        break
    time.sleep(0.1)
else:
    print('timeout waiting for', cmd['rid'])
