const mask = s => typeof s === 'string' ? s[0] + '…(' + s.length + ')' : s;
const ctx = JSON.parse(document.querySelector('shreddit-app').getAttribute('data-faceplate-tracking-context') || '{}');
const u = ctx.user || {};
return { ctxKeys: Object.keys(ctx), userKeys: Object.keys(u), name: mask(u.name), id: mask(u.id), loggedIn: u.logged_in };
