// Agora's bridge, injected into the isolated script world "agora" (SPEC §4.2).
// Page scripts cannot reach `window.agora` or its message handler. Operations
// live in ops/*.js. Never wait on timers here: the hidden view throttles them.
(() => {
  const CHUNK = 60 * 1024;
  const PARTIAL_ACCEPT = 'text/vnd.reddit.partial+html, text/html;q=0.9';

  // Bodies over one chunk travel through the message handler (SPEC §4.2).
  function postChunks(id, text) {
    const count = Math.ceil(text.length / CHUNK);
    for (let i = 0; i < count; i++) {
      window.webkit.messageHandlers.agora.postMessage({
        id,
        data: text.slice(i * CHUNK, (i + 1) * CHUNK),
      });
    }
    return count;
  }

  // URLs from Reddit's HTML come through here, and POSTs carry the CSRF token,
  // so nothing leaves this origin.
  function sameOrigin(url) {
    try {
      return new URL(url, location.origin).origin === location.origin;
    } catch {
      return false;
    }
  }

  async function request(id, url, init, keepBody = true) {
    if (!sameOrigin(url)) {
      return { status: 0, finalUrl: '', error: 'refused a URL on another site' };
    }
    let response;
    try {
      response = await fetch(url, Object.assign({ credentials: 'include', redirect: 'follow' }, init));
    } catch (e) {
      return { status: 0, finalUrl: '', error: String(e) };
    }
    const base = { status: response.status, finalUrl: response.url };
    if (!keepBody && response.ok) {
      response.body?.cancel();
      return base;
    }
    const text = await response.text();
    if (text.length <= CHUNK) {
      return Object.assign(base, { body: text });
    }
    return Object.assign(base, { chunks: postChunks(id, text) });
  }

  function csrf() {
    const match = document.cookie.match(/(?:^|;\s*)csrf_token=([^;]+)/);
    return match ? decodeURIComponent(match[1]) : '';
  }

  window.agora = {
    ops: {},
    // A full page, with the Accept header a navigation would send.
    page: (id, url, keepBody) => request(id, url, {}, keepBody),
    // A partial, with the header `faceplate-partial` sends.
    partial: (id, url) => request(id, url, { headers: { Accept: PARTIAL_ACCEPT } }),
    // A form POST with the CSRF token added.
    partialPost: (id, url, form) => request(id, url, {
      method: 'POST',
      headers: { Accept: PARTIAL_ACCEPT },
      body: new URLSearchParams(Object.assign({}, form, { csrf_token: csrf() })),
    }),
    run(op, args, id) {
      const fn = this.ops[op];
      return fn ? fn(args, id) : { status: 0, finalUrl: '', error: `unknown operation ${op}` };
    },
  };
})();
