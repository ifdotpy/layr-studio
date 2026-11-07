import { Drawer, Badge, Button, NativeSelect } from "@mantine/core";
import { Activity, Terminal } from "lucide-react";
import { useEffect } from "react";
import { observer, Observer } from "mobx-react-lite";
import {
  useBackgroundTaskArea,
  type BackgroundTaskProps,
  BackgroundTaskActionsProvider,
  useBackgroundTaskActions,
} from "./backgroundTaskActions";
import { BackgroundTaskList } from "./BackgroundTaskList";
import {
  ProcessControlActions,
  ProcessControlResults,
} from "./ProcessControlActions";
import { TaskDetail } from "./TaskDetail";
import "./background-controls.css";
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
  useEffect(() => actions.retainSelection(task), [actions, task]);
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
export default function BackgroundTasks(props: BackgroundTaskProps) {
  const actions = useBackgroundTaskArea(props);
  useEffect(() => {
    actions.disposed = false;
    return () => {
      actions.disposed = true;
    };
  }, [actions]);
  useEffect(() => {
    if (!props.opened) actions.back();
  }, [actions, props.opened]);
  useEffect(() => {
    if (
      props.opened &&
      props.initialFocus &&
      props.initialFocus.leadId === props.leadId
    )
      actions.focus(props.initialFocus.id);
  }, [actions, props.opened, props.initialFocus?.requestId]);
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
            <Observer>
              {() => (
                <Badge variant="light" color="gray" size="sm">
                  {actions.activities.length} active
                </Badge>
              )}
            </Observer>
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
}
