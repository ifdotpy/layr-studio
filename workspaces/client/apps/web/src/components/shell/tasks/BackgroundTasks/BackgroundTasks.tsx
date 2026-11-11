import { Drawer, Badge, Button, NativeSelect } from "@mantine/core";
import { Activity, Terminal } from "lucide-react";
import { observer, Observer } from "mobx-react-lite";
import {
  type BackgroundTaskProps,
  type BackgroundTaskActions,
  BackgroundTaskActionsProvider,
  useBackgroundTaskActions,
} from "../support/backgroundTaskActions.tsx";
import { BackgroundTaskList } from "../BackgroundTaskList/BackgroundTaskList.tsx";
import { ProcessControlActions } from "../ProcessControlActions/ProcessControlActions.tsx";
import { ProcessControlResults } from "../ProcessControlResults/ProcessControlResults.tsx";
import { TaskDetail } from "../TaskDetail/TaskDetail.tsx";

const ActivityFilters = observer(function ActivityFilters() {
  const actions = useBackgroundTaskActions();
  return (
    <div className="tasks-toolbar">
      <div className="tasks-filters">
        <NativeSelect
          aria-label="Task type"
          value={actions.kind}
          onChange={(e) => actions.filter(e.target.value, actions.query)}
          data={[
            { value: "all", label: "All" },
            { value: "command", label: "Commands" },
            { value: "monitor", label: "Monitors" },
          ]}
          size="xs"
        />
      </div>
      <span className="tasks-filter-help">
        Commands run in the chat. Monitors run commands in the background.
      </span>
      <ProcessControlActions />
    </div>
  );
});
const ActivityBody = observer(function ActivityBody() {
  const actions = useBackgroundTaskActions();
  const task = actions.selectedEntry?.value;
  return (
    <div
      className={
        "tasks-content " +
        (actions.mobileDetail && (task || actions.explicitId)
          ? "show-task-detail"
          : "")
      }
    >
      <BackgroundTaskList />
      {task ? (
        <TaskDetail key={task.id} task={task} />
      ) : (
        <div className="task-detail-empty">
          <Terminal size={32} />
          <p role="status">
            {actions.explicitId
              ? "The selected task is no longer active in this chat."
              : "Select a task to inspect its output."}
          </p>
          {actions.explicitId && (
            <Button variant="subtle" onClick={actions.back}>
              Back to tasks
            </Button>
          )}
        </div>
      )}
    </div>
  );
});
export default observer(function BackgroundTasks(
  props: BackgroundTaskProps & { actions: BackgroundTaskActions | undefined },
) {
  const { actions } = props;
  if (!actions) return null;
  return (
    <BackgroundTaskActionsProvider value={actions}>
      <Drawer
        opened={props.opened}
        onClose={props.close}
        {...(props.afterClose ? { onExitTransitionEnd: props.afterClose } : {})}
        returnFocus={!props.afterClose}
        position="right"
        size={940}
        padding={0}
        title={
          <div className="tasks-title">
            <Activity size={19} />
            <strong>Current activity</strong>
            {props.opened && (
              <Observer>
                {() => (
                  <Badge variant="light" color="gray" size="sm">
                    {actions.activities.length} active
                  </Badge>
                )}
              </Observer>
            )}
          </div>
        }
        classNames={{
          content: "tasks-drawer",
          body: "tasks-drawer-body",
          header: "tasks-drawer-header",
        }}
      >
        {props.opened && (
          <>
            <ActivityFilters />
            <ProcessControlResults />
            <ActivityBody />
          </>
        )}
      </Drawer>
    </BackgroundTaskActionsProvider>
  );
});

export {
  createBackgroundTaskArea,
  type BackgroundTaskActions,
} from "../support/backgroundTaskActions.tsx";
