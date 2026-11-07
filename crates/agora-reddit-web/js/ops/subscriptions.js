// subscriptions v1: the left-nav partial. Its URL is signed, so it is read from
// the page the hidden view sits on.
agora.ops.subscriptions = (_, id) => {
  const partial = document.querySelector('faceplate-partial[src*="common-left-nav"]');
  if (!partial) {
    return { status: 200, finalUrl: location.href, body: '' };
  }
  return agora.partial(id, partial.getAttribute('src'));
};
