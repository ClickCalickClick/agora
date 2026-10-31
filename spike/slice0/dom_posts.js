const posts = [...document.querySelectorAll('shreddit-post')];
const attrs = posts[0] ? posts[0].getAttributeNames() : [];
return {
  count: posts.length,
  attrs,
  sample: posts.slice(0, 3).map(p => Object.fromEntries(attrs.filter(a => !/author|user/i.test(a)).map(a => [a, (p.getAttribute(a) || '').slice(0, 120)]))),
  textBody: posts[0] ? !!posts[0].querySelector('[slot="text-body"]') : null,
  moreLoaderUrls: [...document.querySelectorAll('faceplate-partial[src], [src*="/svc/shreddit/"]')].slice(0, 8).map(e => e.getAttribute('src')),
};
