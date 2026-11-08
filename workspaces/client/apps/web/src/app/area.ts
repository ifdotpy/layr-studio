import { createContext, useContext, type Provider } from "react";

/** One command context for an area; no registration or lifecycle ownership. */
export function createArea<T>(name: string): [Provider<T | null>, () => T] {
  const Context = createContext<T | null>(null);
  function useArea(): T {
    const value = useContext(Context);
    if (value === null)
      throw new Error(`${name} must be used inside its provider`);
    return value;
  }
  return [Context.Provider, useArea];
}
