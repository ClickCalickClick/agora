// more_comments v1: POST to the partial's own src with its hidden cursor.
agora.ops.more_comments = ({ src, cursor }, id) => agora.partialPost(id, src, { cursor });
