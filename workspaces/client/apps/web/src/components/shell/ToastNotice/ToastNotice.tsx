import { errorText } from "../../../api.ts";
import { Button } from "@mantine/core";
import { observer } from "mobx-react-lite";
import { useServerAreas } from "../../../app/ServerAreasProvider.tsx";

export const ToastNotice = observer(function ToastNotice() {
  const { notices, delivery, scope } = useServerAreas();
  const requests = scope.services.requests;
  return (
    <>
      {[...requests.entries.values()].map((entry) => {
        if (entry.terminal) return null;
        const view = entry.view;
        return (
          view.errorText && (
            <div
              key={entry.requestId}
              role="status"
              className="native-action-pending"
            >
              <span>
                {entry.intent.label}: {view.errorText}
              </span>
              {view.canRetry && (
                <Button
                  onClick={() =>
                    void requests
                      .retry(entry.requestId)
                      .catch((error) => notices.notify(errorText(error)))
                  }
                >
                  Retry saved request
                </Button>
              )}
            </div>
          )
        );
      })}
      {notices.toast && !delivery.ownsOfflineError(notices.toast) && (
        <div id="toast" role="status">
          {notices.toast}
        </div>
      )}
    </>
  );
});
