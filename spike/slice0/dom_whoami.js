const mask = s => s ? s[0] + '…(' + s.length + ')' : s;
const app = document.querySelector('shreddit-app');
const appAttrs = app ? app.getAttributeNames() : [];
const userAttrs = {};
for (const a of appAttrs) if (/user|name|logged/i.test(a)) userAttrs[a] = mask(app.getAttribute(a));
const links = [...document.querySelectorAll('header a[href^="/user/"], [slot] a[href^="/user/"]')].slice(0,3).map(a => mask(a.getAttribute('href').split('/')[2]));
const html = document.documentElement.outerHTML;
const m = html.match(/"(?:username|displayName|name)":"([^"]{3,20})","(?:[a-zA-Z]+)":/);
return { appAttrs, userAttrs, links, jsonHit: m ? mask(m[1]) : null, loggedIn: document.querySelector('shreddit-post')?.getAttribute('user-logged-in') };
