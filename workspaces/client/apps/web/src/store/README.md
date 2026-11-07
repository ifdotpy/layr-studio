# Observable resources

`index.ts` exports the small runtime used by generated resource facades. A server
creates one runtime with its existing subscription module. Components consume
generated facades and readonly `ResourceSlot.current` values; mutable models and
their `apply` methods stay private to generated code.

Each slot tracks observation through its own atom. `interest.ts` waits for 100 ms
of continuous observation before acquiring a reference. The existing subscription
module owns reference counting, immediate stale status, transport batching,
20-second grace, and its 128-entry inactive cache bound. This module owns none of
the SSE transport, cursor, epoch, tombstone, or server identity policy.

Subscription notifications coalesce in one microtask and apply inside one MobX
action. Actions reading a facade outside an observer acquire no interest. An
abandoned React render can retain a reaction until mobx-react-lite finalization;
the reference is then released through the same subscription grace.
