const tags = new Set([...document.querySelectorAll('*')].map(e => e.tagName.toLowerCase()).filter(t => /subreddit|community|rule|sidebar|about/.test(t)));
const hdr = document.querySelector('shreddit-subreddit-header');
const srcs = [...document.querySelectorAll('faceplate-partial[src]')].map(e => e.getAttribute('src')).filter(s => /subreddit|community|rule|sidebar|about|widget/i.test(s));
return { tags: [...tags], hdr: hdr ? Object.fromEntries(hdr.getAttributeNames().map(a => [a, (hdr.getAttribute(a)||'').slice(0,100)])) : null, srcs };
