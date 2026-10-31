#!/usr/bin/env python3
"""Turn a raw HTML partial into a small, scrubbed fixture.
   fixture.py RAW OUT --user NAME [--posts N] [--comments N]
Drops script/style/svg, keeps the first N posts and N top-level comment
subtrees, replaces every author with a pseudonym and redacts ids/tokens."""
import argparse, html, re
from html.parser import HTMLParser

ap = argparse.ArgumentParser()
ap.add_argument('raw'); ap.add_argument('out')
ap.add_argument('--user', required=True)
ap.add_argument('--posts', type=int, default=3)
ap.add_argument('--comments', type=int, default=5)
a = ap.parse_args()

VOID = {'area', 'base', 'br', 'col', 'embed', 'hr', 'img', 'input', 'link', 'meta', 'source', 'track', 'wbr'}
DROP = {'script', 'style', 'svg', 'faceplate-loader', 'noscript'}

class Trim(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=False)
        self.out, self.skip, self.skiptag = [], 0, None
        self.posts = self.comments = 0
    def drop_start(self, tag, attrs):
        if tag in DROP:
            return True
        if tag in ('shreddit-post', 'shreddit-ad-post'):
            if tag == 'shreddit-ad-post':
                return True
            self.posts += 1
            return self.posts > a.posts
        if tag == 'shreddit-comment' and dict(attrs).get('depth') == '0':
            self.comments += 1
            return self.comments > a.comments
        return False
    def handle_starttag(self, tag, attrs):
        if self.skip:
            if tag == self.skiptag and tag not in VOID:
                self.skip += 1
            return
        if self.drop_start(tag, attrs):
            if tag not in VOID:
                self.skip, self.skiptag = 1, tag
            return
        self.out.append(self.get_starttag_text())
    def handle_startendtag(self, tag, attrs):
        if not self.skip and tag not in DROP:
            self.out.append(self.get_starttag_text())
    def handle_endtag(self, tag):
        if self.skip:
            if tag == self.skiptag:
                self.skip -= 1
            return
        self.out.append(f'</{tag}>')
    def handle_data(self, d):
        if not self.skip:
            self.out.append(d)
    def handle_entityref(self, n):
        if not self.skip: self.out.append(f'&{n};')
    def handle_charref(self, n):
        if not self.skip: self.out.append(f'&#{n};')
    def handle_comment(self, d):
        pass
    def handle_decl(self, d):
        self.out.append(f'<!{d}>')

raw = open(a.raw).read()
t = Trim(); t.feed(raw); t.close()
s = ''.join(t.out)
s = re.sub(r'\n\s*\n+', '\n', s)

# Usernames: every author/user name seen anywhere in the raw body.
names = set(re.findall(r'\bauthor="([^"]+)"', raw))
names |= set(re.findall(r'/user/([A-Za-z0-9_-]{3,20})', raw))
names |= set(re.findall(r'user-hover-card/([A-Za-z0-9_-]{3,20})', raw))
names |= set(re.findall(r'Comment from ([A-Za-z0-9_-]{3,20})', raw))
names |= {a.user}
names -= {'me', 'AutoModerator', '[deleted]'}
mapping = {a.user.lower(): 'agora_tester'}
for n in sorted(names, key=lambda n: (-len(n), n)):
    mapping.setdefault(n.lower(), f'user{len(mapping)}')
s = re.sub(re.escape(a.user), 'agora_tester', s, flags=re.I)
for n in sorted(mapping, key=len, reverse=True):
    s = re.sub(r'(?:(?<![A-Za-z0-9_-])|(?<=%3D)|(?<=%2F))' + re.escape(n) + r'(?![A-Za-z0-9_-])', mapping[n], s, flags=re.I)

# Ids and tokens.
s = re.sub(r'\b(author-id|user-id|loid|correlation-id|serverrenderid|feed-correlation-id|profile-correlation-id|'
           r'navigation-session-id|platform-health-id|clienthash|country|ipv4-probe-url)="[^"]*"', r'\1="REDACTED"', s)
s = re.sub(r't2_[a-z0-9]+', 't2_REDACTED', s)
s = re.sub(r'(sig=v1\.)[A-Za-z0-9_-]+', r'\1REDACTED', s)
s = re.sub(r'(csrf_token|modhash|token)(["\'=:\s]+)[A-Za-z0-9._%-]{8,}', r'\1\2REDACTED', s, flags=re.I)
s = re.sub(r'(data-faceplate-tracking-context)="[^"]*"', r'\1="REDACTED"', s)
# Avatars identify accounts by image; point them at a placeholder.
s = re.sub(r'https://(?:styles\.redditmedia\.com|www\.redditstatic\.com/avatars)/[^"\s)]+', 'https://example.invalid/avatar.png', s)

assert a.user.lower() not in s.lower(), 'account name survived'
open(a.out, 'w').write(s)
print(f'{a.out}: {len(raw)} -> {len(s)} bytes, posts kept {min(t.posts, a.posts)}, top comments kept {min(t.comments, a.comments)}, names scrubbed {len(mapping)}')
