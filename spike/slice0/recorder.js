// Spike-only: page-world recorder of the front end's own write requests,
// used while the user clicks by hand. Records shape only; secrets redacted.
(() => {
  const rec = window.__agoraRec = [];
  const SECRET = /csrf|token|modhash|uh$/i;
  function shape(body) {
    if (body == null) return null;
    if (typeof body === 'string') {
      try { return redact(JSON.parse(body)); } catch (e) {}
      try { return redact(Object.fromEntries(new URLSearchParams(body))); } catch (e) {}
      return { kind: 'string', length: body.length };
    }
    if (body instanceof FormData || body instanceof URLSearchParams) return redact(Object.fromEntries(body.entries()));
    return { kind: Object.prototype.toString.call(body) };
  }
  function redact(o) {
    if (Array.isArray(o)) return o.map(redact);
    if (o && typeof o === 'object') {
      const out = {};
      for (const [k, v] of Object.entries(o)) out[k] = SECRET.test(k) ? 'REDACTED' : redact(v);
      return out;
    }
    return o;
  }
  const keep = (m, u) => m !== 'GET' && !/google|doubleclick|events|w3-reporting|error-reporting|telemetry/.test(u);
  const of = window.fetch;
  window.fetch = function (input, init) {
    const url = typeof input === 'string' ? input : input.url;
    const method = ((init && init.method) || (input && input.method) || 'GET').toUpperCase();
    const headers = init && init.headers ? Object.keys(init.headers instanceof Headers ? Object.fromEntries(init.headers) : init.headers) : [];
    const p = of.apply(this, arguments);
    if (keep(method, url)) {
      const entry = { via: 'fetch', method, url, headers, body: shape(init && init.body) };
      rec.push(entry);
      p.then(r => { entry.status = r.status; }).catch(() => {});
    }
    return p;
  };
  const oo = XMLHttpRequest.prototype.open, os = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (m, u) { this.__r = { via: 'xhr', method: m.toUpperCase(), url: u }; return oo.apply(this, arguments); };
  XMLHttpRequest.prototype.send = function (b) {
    if (this.__r && keep(this.__r.method, this.__r.url)) {
      this.__r.body = shape(b); rec.push(this.__r);
      this.addEventListener('load', () => { this.__r.status = this.status; });
    }
    return os.apply(this, arguments);
  };
})();
