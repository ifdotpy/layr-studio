import type { post, PostInput, PostOptions, PostResult } from "../api.ts";

type PostPath = Parameters<typeof post>[0];
type PostHandlers = {
  [P in PostPath]?: (
    body: PostInput<P>,
    options?: PostOptions,
  ) => Promise<PostResult<P>>;
};

/** Test double for the generic POST transport: each path answers through its own handler. */
export function postDouble(handlers: PostHandlers): typeof post {
  return (path, body, options) => {
    const handler = handlers[path];
    if (!handler) throw new Error(`No test handler for ${path}.`);
    return handler(body, options);
  };
}
