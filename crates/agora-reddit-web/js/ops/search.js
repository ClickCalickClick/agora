// search v1: the results page; `after` is the next-page partial it names.
agora.ops.search = ({ query, subreddit, sort, range, after }, id) => {
  if (after) {
    return agora.partial(id, after);
  }
  const scope = subreddit ? `/r/${encodeURIComponent(subreddit)}` : '';
  const q = encodeURIComponent(query);
  return agora.page(id, `${scope}/search/?q=${q}&type=posts&sort=${sort}&t=${range}`);
};
