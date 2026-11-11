import { useState } from "react";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";

/** An action-selected value stays with its server owner until closed or replaced. */
// integration: foreign callers move their selections into Owned stores.
export function useReadView<T>(initial: T | null = null) {
  const owner = useServerStore();
  const [selection, setSelection] = useState(() => ({ owner, value: initial }));
  return [
    selection.owner === owner ? selection.value : null,
    (value: T | null) => setSelection({ owner, value }),
  ] as const;
}
