import { observer } from "mobx-react-lite";
import { NativeError } from "../../shell/NativeNotice/NativeNotice.tsx";

export const ObservedNativeError = observer(NativeError);
