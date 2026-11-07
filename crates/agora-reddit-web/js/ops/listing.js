// listing v1: the *-more-posts and feed partials. `after` is the whole next-page
// URL the previous page named.
agora.ops.listing = ({ feed, name, sort, range, after }, id) => {
  if (after) {
    return agora.partial(id, after);
  }
  const t = sort === 'top' ? `&t=${range.toUpperCase()}` : '';
  const n = encodeURIComponent(name || '');
  switch (feed) {
    case 'home':
      return agora.partial(id, `/svc/shreddit/feeds/home-feed?sort=${sort.toUpperCase()}${t}`);
    case 'popular':
      return agora.partial(id, `/svc/shreddit/feeds/popular-feed?sort=${sort.toUpperCase()}${t}`);
    case 'subreddit':
      return agora.partial(id, `/svc/shreddit/community-more-posts/${sort}/?name=${n}${t}`);
    case 'user':
      return agora.partial(id, `/svc/shreddit/profiles/profile_submitted-more-posts/${sort}/?name=${n}${t}`);
    case 'saved':
      return agora.partial(id, `/svc/shreddit/profiles/profile_saved-more-posts/new/?name=${n}`);
    default:
      return { status: 0, finalUrl: '', error: `unknown feed ${feed}` };
  }
};
