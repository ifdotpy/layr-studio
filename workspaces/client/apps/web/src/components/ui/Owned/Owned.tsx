import { Component, type ReactNode } from "react";
import { Observer } from "mobx-react-lite";

/**
 * Owns one MobX store for the lifetime of this mounted element, without hooks.
 * `create` runs once per mount (change the element `key` to get a fresh store);
 * it must be side-effect free. Stores that need disposal belong to a real owner.
 */
export class Owned<S> extends Component<{
  create: () => S;
  children: (store: S) => ReactNode;
}> {
  readonly store = this.props.create();

  override render(): ReactNode {
    return <Observer>{() => this.props.children(this.store)}</Observer>;
  }
}
