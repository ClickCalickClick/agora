const srcs = [...document.querySelectorAll('[src]')].map(e => e.tagName.toLowerCase() + ' ' + e.getAttribute('src')).filter(s => s.includes('/svc/') || s.includes('more'));
const feed = document.querySelector('shreddit-feed');
return { srcs: srcs.slice(0, 30), feedAttrs: feed ? Object.fromEntries(feed.getAttributeNames().map(a => [a, (feed.getAttribute(a)||'').slice(0,200)])) : null,
  articles: document.querySelectorAll('article').length };
