// subscriptions v1: the left-nav partial. Its URL is signed, so it is read from
// the page the hidden view sits on.
agora.ops.subscriptions = (_, id) => {
  const partial = document.querySelector('faceplate-partial[src*="common-left-nav"]');
  const src = partial && partial.getAttribute('src');
  if (!src || !src.startsWith('/svc/shreddit/')) {
    return { status: 200, finalUrl: location.href, body: '' };
  }
  return agora.partial(id, src);
};
