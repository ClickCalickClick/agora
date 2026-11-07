// post_with_comments, first step, v1: the post page holds the <shreddit-post>
// with its text body.
agora.ops.post_page = ({ subreddit, post }, id) =>
  agora.page(id, `/r/${encodeURIComponent(subreddit)}/comments/${encodeURIComponent(post)}/`);
