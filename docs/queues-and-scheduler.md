# Download queues

[← Back to docs index](README.md)

A **queue** downloads files in order, a few at a time. This Windows build retains basic queues
and categories, without the Scheduler or advanced queue automation.

## Put downloads in a queue

- In **Download File Info**, click **Download Later** and pick the queue.
- Or select unfinished downloads, right-click **Add to queue**, and select a queue.
  Downloads already in a queue show **Move to queue** instead.
- **Delete from queue** removes queue membership, not the download or its files.

New installations have a **Main download queue**. Existing saved queues and their memberships
remain available. Select one under **Queues** in the categories pane to see its downloads.
The agent CLI can also manage basic queues.

## Start and stop

- **Start Queue** and **Stop Queue** on the toolbar control the main queue.
  Their arrows list the other queues.
- **Downloads → Start queue** and **Downloads → Stop queue** do the same.
- Or right-click a queue under **Queues** and choose **Start now** or **Stop**.

A running queue starts files in order, four at a time by default. Failed files move to the end
and wait 30 seconds before a retry. Completed downloads leave their queues but remain in the
download list. Manually stopped downloads stay stopped until explicitly started again.

## Older saved settings

Old download lists continue to load. Saved start/stop schedules, startup queue actions, wake
timers, traffic quotas, and completion commands are no longer executed. No automatic shutdown,
sleep, hibernation, or post-download program launch takes place. Old synchronization queues
are treated as basic queues.

Completed archives are kept as downloaded: there is no automatic extraction or archive deletion.
Normal clipboard copy/paste and **Add batch download from clipboard** remain available; the
clipboard is not monitored in the background.

## Next steps

- [Keep some speed for everything else →](guide-keep-speed-for-everything-else.md)
- [The download list →](download-list.md)
