const cs = [...document.querySelectorAll('shreddit-comment')];
const attrs = cs[0] ? cs[0].getAttributeNames() : [];
const keep = attrs.filter(a => !/author|user/i.test(a));
return {
  count: cs.length,
  attrs,
  sample: cs.slice(0, 2).map(c => ({...Object.fromEntries(keep.map(a => [a, (c.getAttribute(a) || '').slice(0, 150)])),
     bodyHtml: (c.querySelector('[slot="comment"]') || {}).innerHTML?.slice(0, 300)})),
  moreSrcs: [...document.querySelectorAll('[src*="more-comments"],[src*="comments"], faceplate-partial[loading]')].map(e => e.tagName.toLowerCase() + ' ' + e.getAttribute('src')).slice(0, 10),
  moreForms: [...document.querySelectorAll('faceplate-partial')].filter(e => /comment/.test(e.getAttribute('src')||'')).map(e => ({src: e.getAttribute('src'), method: e.getAttribute('method'), hasForm: !!e.querySelector('input')})).slice(0, 6),
  postTextBody: !!document.querySelector('shreddit-post [slot="text-body"]'),
};
