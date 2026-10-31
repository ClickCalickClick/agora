const html = document.documentElement.outerHTML;
const m = [...new Set(html.match(/\/svc\/shreddit\/[a-z-]*(more|comments)[^"'\s<>]{0,200}/g) || [])];
const tree = document.querySelector('shreddit-comment-tree');
const parts = [...document.querySelectorAll('shreddit-comment-tree faceplate-partial, shreddit-comment faceplate-partial')].map(e => ({src: e.getAttribute('src'), method: e.getAttribute('method'), loading: e.getAttribute('loading'), inputs: [...e.querySelectorAll('input')].map(i => i.name)}));
return { urls: m.slice(0, 15), treeAttrs: tree ? Object.fromEntries(tree.getAttributeNames().map(a => [a, (tree.getAttribute(a)||'').slice(0,200)])) : null, parts: parts.slice(0, 8) };
