// post_with_comments, second step, v1: the comment tree partial, or one branch.
agora.ops.comments = ({ subreddit, post, sort, focus }, id) => {
  const base = `/svc/shreddit/comments/r/${encodeURIComponent(subreddit)}/${encodeURIComponent(post)}`;
  const path = focus ? `${base}/comment/${encodeURIComponent(focus)}` : base;
  return agora.partial(id, `${path}?render-mode=partial&sort=${sort}`);
};
