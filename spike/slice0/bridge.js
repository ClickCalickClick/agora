// Slice 0 spike bridge. Injected into the isolated script world "agora".
// Throwaway: the real bridge lives in crates/agora-reddit-web from slice 2.
(() => {
  const CHUNK = 60 * 1024;

  function send(id, text) {
    const n = Math.max(1, Math.ceil(text.length / CHUNK));
    for (let i = 0; i < n; i++) {
      window.webkit.messageHandlers.agora.postMessage(
        { id, chunk: i, of: n, data: text.slice(i * CHUNK, (i + 1) * CHUNK) });
    }
    return n;
  }

  async function request(id, method, url, form, headers) {
    const t0 = performance.now();
    let r;
    const init = { method, credentials: 'include', redirect: 'follow', headers: headers || {} };
    if (form instanceof Object && form.__json) {
      init.body = JSON.stringify(form.__json);
      init.headers = Object.assign({ 'content-type': 'application/json', accept: 'application/json' }, init.headers);
    } else if (form) {
      init.body = new URLSearchParams(form);
    }
    try {
      r = await fetch(url, init);
    } catch (e) {
      return { ok: false, error: String(e), ms: Math.round(performance.now() - t0) };
    }
    const text = await r.text();
    return {
      ok: r.ok, status: r.status, url, finalUrl: r.url, redirected: r.redirected,
      contentType: r.headers.get('content-type'),
      ratelimit: {
        used: r.headers.get('x-ratelimit-used'),
        remaining: r.headers.get('x-ratelimit-remaining'),
        reset: r.headers.get('x-ratelimit-reset'),
      },
      ms: Math.round(performance.now() - t0),
      length: text.length,
      chunks: send(id, text),
    };
  }

  // Cookie *names* only; values never leave this function.
  function env() {
    const names = document.cookie.split(';').map(c => c.split('=')[0].trim()).filter(Boolean);
    const csrfInput = document.querySelector('input[name="csrf_token"]');
    const app = document.querySelector('shreddit-app');
    return {
      href: location.href,
      title: document.title,
      cookieNames: names,
      csrfCookieReadable: names.includes('csrf_token'),
      csrfInDom: !!csrfInput,
      shredditApp: !!app,
      shredditAppAttrs: app ? app.getAttributeNames() : [],
      visibilityState: document.visibilityState,
      hidden: document.hidden,
      userAgent: navigator.userAgent,
    };
  }

  function csrfFromCookie() {
    const m = document.cookie.match(/(?:^|;\s*)csrf_token=([^;]+)/);
    return m ? decodeURIComponent(m[1]) : null;
  }

  // Timer drift test: 20 x setTimeout(50 ms). Unthrottled ≈ 1000 ms.
  function timers() {
    return new Promise(resolve => {
      const t0 = performance.now();
      let n = 0;
      const tick = () => {
        if (++n === 20) {
          resolve({ expectedMs: 1000, actualMs: Math.round(performance.now() - t0) });
        } else {
          setTimeout(tick, 50);
        }
      };
      setTimeout(tick, 50);
    });
  }

  window.agora = {
    async run(op, args) {
      switch (op) {
        case 'get':
          return request(args.id, 'GET', args.url, null, args.headers);
        case 'post': {
          const form = Object.assign({}, args.form || {});
          const headers = Object.assign({}, args.headers || {});
          if (args.csrf === 'cookie-form') {
            form.csrf_token = csrfFromCookie();
          }
          if (args.json) {
            const body = Object.assign({}, args.json, { csrf_token: csrfFromCookie() });
            return request(args.id, 'POST', args.url, { __json: body }, headers);
          }
          return request(args.id, 'POST', args.url, form, headers);
        }
        case 'env':
          return env();
        case 'timers':
          return timers();
        default:
          return { ok: false, error: 'unknown op ' + op };
      }
    },
  };
})();
