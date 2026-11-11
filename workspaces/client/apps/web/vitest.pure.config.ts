import base from "./vitest.unit.config.ts";
import { HTTP_FIXTURE_TESTS } from "./vitest.http.config.ts";

export default {
  ...base,
  test: {
    ...base.test,
    exclude: [...(base.test?.exclude ?? []), ...HTTP_FIXTURE_TESTS],
  },
};
