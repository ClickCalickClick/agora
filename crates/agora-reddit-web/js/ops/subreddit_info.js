// subreddit_info v1: the right-rail partial.
agora.ops.subreddit_info = ({ name }, id) =>
  agora.partial(id, `/svc/shreddit/feeds/subreddit-right-rail?name=${encodeURIComponent(name)}`);
