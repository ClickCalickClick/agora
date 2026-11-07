// user_profile v1: the profile page.
agora.ops.user_profile = ({ name }, id) => agora.page(id, `/user/${encodeURIComponent(name)}/`);
