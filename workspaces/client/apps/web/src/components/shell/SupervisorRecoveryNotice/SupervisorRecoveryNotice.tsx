import { observer } from "mobx-react-lite";
import { resourceData } from "../../../store/runtime.ts";
import { useServerStore } from "../../servers/ServerServicesProvider/ServerServicesProvider.tsx";
import { desktopNotices } from "./desktopNotices.ts";

// Host notices from /api/desktop: supervisor recovery and low disk space.
export default observer(function SupervisorRecoveryNotice() {
  const desktop = resourceData(useServerStore().desktop().current);
  const notices = desktop ? desktopNotices(desktop) : [];

  return notices.map((notice) => (
    <div
      key={notice.id}
      className="supervisor-recovery-notice"
      data-level={notice.level}
      title={notice.text}
      aria-label={notice.text}
      role="alert"
    >
      {notice.text}
    </div>
  ));
});
