const tags = [...new Set([...document.querySelectorAll('*')].map(e => e.tagName.toLowerCase()).filter(t => t.includes('-')))];
const main = document.querySelector('main') || document.body;
return { tags: tags.filter(t => !/^(faceplate|svg|rpl)-/.test(t)).slice(0, 80), mainText: main.innerText.replace(/\s+/g, ' ').slice(0, 600) };
