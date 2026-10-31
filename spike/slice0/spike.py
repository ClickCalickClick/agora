#!/usr/bin/env python3
"""Slice 0 spike driver: a hidden WebKitGTK 6.0 view on www.reddit.com with a
JS bridge in the isolated world "agora", driven one command at a time through
a FIFO by ctl.py. Throwaway code; see docs/SPIKE-0.md.

Safety rules baked in:
- one request at a time, >= 1.0 s + 0-500 ms jitter between network requests;
- any 429, captcha/interstitial or redirect to login latches BLOCKED and every
  later network command is refused until the process restarts;
- the password is typed by the user into Reddit's own login page; cookies stay
  in WebKit's profile and are never read or printed by this script.
"""
import json, os, random, sys, time
import gi
gi.require_version('Gtk', '4.0')
gi.require_version('WebKit', '6.0')
gi.require_version('Soup', '3.0')
from gi.repository import GLib, Gtk, WebKit, Soup

BASE = os.path.expanduser('~/.local/share/agora-spike')
RAW = os.path.join(BASE, 'raw')            # full response bodies, never committed
FIFO = os.path.join(BASE, 'ctl.fifo')
OUT = os.path.join(BASE, 'out')            # per-command result JSON
HERE = os.path.dirname(os.path.abspath(__file__))
UA = 'Agora/0.0-spike (GNOME; +https://github.com/ClickCalickClick/agora)'
MIN_GAP, JITTER = 1.0, 0.5

for d in (RAW, OUT, os.path.join(BASE, 'web'), os.path.join(BASE, 'cache')):
    os.makedirs(d, exist_ok=True)


class Spike:
    def __init__(self, app):
        self.app = app
        self.blocked = None
        self.last_net = 0.0
        self.chunks = {}
        self.session = WebKit.NetworkSession.new(os.path.join(BASE, 'web'), os.path.join(BASE, 'cache'))
        self.session.get_cookie_manager().set_persistent_storage(
            os.path.join(BASE, 'web', 'cookies.sqlite'), WebKit.CookiePersistentStorage.SQLITE)

        ucm = WebKit.UserContentManager()
        with open(os.path.join(HERE, 'bridge.js')) as f:
            ucm.add_script(WebKit.UserScript.new_for_world(
                f.read(), WebKit.UserContentInjectedFrames.TOP_FRAME,
                WebKit.UserScriptInjectionTime.START, 'agora', None, None))
        ucm.register_script_message_handler('agora', 'agora')
        ucm.connect('script-message-received::agora', self.on_message)
        self.view = WebKit.WebView(network_session=self.session, user_content_manager=ucm)
        self.view.connect('load-changed', self.on_load)
        self.load_cb = None
        self.mapped_win = None
        self.login_win = None
        self.soup = Soup.Session(user_agent=UA)

        if os.path.exists(FIFO):
            os.unlink(FIFO)
        os.mkfifo(FIFO)
        self.fd = os.open(FIFO, os.O_RDWR | os.O_NONBLOCK)
        self.buf = b''
        GLib.io_add_watch(self.fd, GLib.IO_IN, self.on_fifo)
        print('spike ready; fifo', FIFO, flush=True)

    # --- plumbing -----------------------------------------------------------
    def reply(self, rid, obj):
        obj['rid'] = rid
        obj['blocked'] = self.blocked
        tmp = os.path.join(OUT, rid + '.tmp')
        with open(tmp, 'w') as f:
            json.dump(obj, f, indent=1)
        os.rename(tmp, os.path.join(OUT, rid + '.json'))

    def on_fifo(self, fd, cond):
        self.buf += os.read(fd, 65536)
        while b'\n' in self.buf:
            line, self.buf = self.buf.split(b'\n', 1)
            if line.strip():
                cmd = json.loads(line)
                try:
                    self.dispatch(cmd)
                except Exception as e:
                    self.reply(cmd['rid'], {'error': repr(e)})
        return True

    def paced(self, rid, fn):
        """Run fn after the pacing gap; refuse if blocked."""
        if self.blocked:
            self.reply(rid, {'error': 'refused: blocked', 'reason': self.blocked})
            return
        wait = self.last_net + MIN_GAP + random.uniform(0, JITTER) - time.monotonic()
        def go():
            self.last_net = time.monotonic()
            fn()
            return False
        GLib.timeout_add(max(0, int(wait * 1000)), go)

    def check_pushback(self, res, body):
        status = res.get('status')
        final = (res.get('finalUrl') or '')
        low = body[:20000].lower() if body else ''
        reason = None
        if status == 429:
            reason = 'HTTP 429'
        elif '/login' in final or '/account/login' in final:
            reason = 'redirect to login: ' + final
        elif 'captcha' in low and 'json' not in (res.get('contentType') or ''):
            reason = 'captcha marker in HTML body'
        elif status == 403 and 'blocked by network security' in (body or '').lower():
            reason = 'HTTP 403 "blocked by network security" page'
        if reason:
            self.blocked = reason
            print('BLOCKED:', reason, flush=True)

    def on_message(self, ucm, value):
        msg = json.loads(value.to_json(0))
        parts = self.chunks.setdefault(msg['id'], {})
        parts[msg['chunk']] = msg['data']

    def take_body(self, rid):
        parts = self.chunks.pop(rid, {})
        return ''.join(parts[i] for i in sorted(parts))

    def js(self, body, args, world, cb):
        variant = GLib.Variant('a{sv}', {k: GLib.Variant('s', v) for k, v in args.items()})
        def done(view, result):
            try:
                v = view.call_async_javascript_function_finish(result)
                cb(json.loads(v.to_json(0)) if not v.is_undefined() else None, None)
            except GLib.Error as e:
                cb(None, e.message)
        self.view.call_async_javascript_function(body, -1, variant, world, None, None, done)

    def on_load(self, view, event):
        if event == WebKit.LoadEvent.FINISHED and self.load_cb:
            cb, self.load_cb = self.load_cb, None
            cb()

    # --- commands -----------------------------------------------------------
    def dispatch(self, cmd):
        rid, c = cmd['rid'], cmd['cmd']
        if c == 'login':
            self.show_login(rid)
        elif c == 'close_login':
            if self.login_win:
                self.login_win.destroy()
                self.login_win = None
            self.reply(rid, {'ok': True})
        elif c == 'home':
            def nav():
                t0 = time.monotonic()
                def loaded():
                    uri = self.view.get_uri()
                    res = {'requested': cmd['url'], 'uri': uri, 'ms': int((time.monotonic() - t0) * 1000)}
                    if '/login' in (uri or ''):
                        self.blocked = 'home redirected to login: ' + uri
                    self.reply(rid, res)
                self.load_cb = loaded
                self.view.load_uri(cmd['url'])
            self.paced(rid, nav)
        elif c == 'op':
            op, args = cmd['op'], dict(cmd.get('args', {}))
            args['id'] = rid
            def run():
                t0 = time.monotonic()
                def done(res, err):
                    out = {'op': op, 'wallMs': int((time.monotonic() - t0) * 1000), 'result': res, 'error': err}
                    if op in ('get', 'post') and res:
                        body = self.take_body(rid)
                        with open(os.path.join(RAW, rid + '.body'), 'w') as f:
                            f.write(body)
                        self.check_pushback(res, body)
                        out['preview'] = body[:600]
                    self.reply(rid, out)
                self.js('return agora.run(op, JSON.parse(args));',
                        {'op': op, 'args': json.dumps(args)}, cmd.get('world', 'agora'), done)
            if op in ('get', 'post'):
                self.paced(rid, run)
            else:
                run()
        elif c == 'jsraw':
            # DOM reads only (no network): body runs in the agora world.
            self.js(cmd['body'], {}, cmd.get('world', 'agora'),
                    lambda res, err: self.reply(rid, {'result': res, 'error': err}))
        elif c == 'recorder':
            with open(os.path.join(HERE, 'recorder.js')) as f:
                self.view.get_user_content_manager().add_script(WebKit.UserScript.new(
                    f.read(), WebKit.UserContentInjectedFrames.TOP_FRAME,
                    WebKit.UserScriptInjectionTime.START, None, None))
            self.reply(rid, {'ok': True})
        elif c == 'pageworld_check':
            body = ('return {agora: typeof window.agora, '
                    'handler: typeof (window.webkit && window.webkit.messageHandlers '
                    '&& window.webkit.messageHandlers.agora)};')
            self.js(body, {}, None, lambda res, err: self.reply(rid, {'pageWorld': res, 'error': err}))
        elif c == 'map':
            if cmd.get('on'):
                self.mapped_win = Gtk.Window(application=self.app, title='Agora spike (mapped view)')
                self.mapped_win.set_default_size(800, 600)
                self.mapped_win.set_child(self.view)
                self.mapped_win.present()
            elif self.mapped_win:
                self.mapped_win.set_child(None)
                self.mapped_win.destroy()
                self.mapped_win = None
            self.reply(rid, {'ok': True, 'mapped': bool(self.mapped_win)})
        elif c == 'cdn':
            def fetch():
                msg = Soup.Message.new('GET', cmd['url'])
                t0 = time.monotonic()
                def done(sess, result):
                    try:
                        data = sess.send_and_read_finish(result)
                        self.reply(rid, {'status': msg.get_status(), 'contentType':
                                         msg.get_response_headers().get_one('content-type'),
                                         'bytes': data.get_size(), 'ms': int((time.monotonic() - t0) * 1000),
                                         'ua': UA})
                    except GLib.Error as e:
                        self.reply(rid, {'status': msg.get_status(), 'error': e.message})
                self.soup.send_and_read_async(msg, GLib.PRIORITY_DEFAULT, None, done)
            self.paced(rid, fetch)
        elif c == 'quit':
            self.reply(rid, {'ok': True})
            self.app.quit()
        else:
            self.reply(rid, {'error': 'unknown cmd ' + c})

    def show_login(self, rid):
        # Visible view, same NetworkSession, no bridge. The user signs in on
        # Reddit's own page; this script never touches the form.
        self.login_win = Gtk.Window(application=self.app, title='Agora spike: sign in to Reddit')
        self.login_win.set_default_size(520, 760)
        lv = WebKit.WebView(network_session=self.session)
        self.login_win.set_child(lv)
        lv.load_uri('https://www.reddit.com/login/')
        self.login_win.present()
        self.reply(rid, {'ok': True})


def main():
    app = Gtk.Application(application_id='io.github.ClickCalickClick.Agora.Spike')
    holder = {}
    def activate(a):
        a.hold()
        holder['s'] = Spike(a)
    app.connect('activate', activate)
    app.run([])

if __name__ == '__main__':
    main()
