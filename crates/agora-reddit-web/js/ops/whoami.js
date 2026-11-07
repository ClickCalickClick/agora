// whoami v1: /user/me/ redirects to /user/{name}/; only the URL is read.
agora.ops.whoami = (_, id) => agora.page(id, '/user/me/', false);
