function getTodayTasks() {
  const today = Utilities.formatDate(new Date(), 'Asia/Tokyo', 'yyyy-MM-dd');
  const result = [];
  const taskLists = fetchAllTaskLists_();

  taskLists.forEach(list => {
    const tasks = fetchAllTasksInList_(list.id, {
      showCompleted: false,
      showHidden: true,
      showDeleted: false
    });

    const todayTasks = tasks
      .filter(task => task.status !== 'completed')
      .filter(task => task.due)
      .filter(task => String(task.due).slice(0, 10) === today)
      .map(task => ({
        taskListId: list.id,
        taskListName: list.title,
        taskId: task.id,
        title: task.title || '',
        notes: task.notes || '',
        due: task.due,
        status: task.status
      }));

    if (todayTasks.length > 0) {
      result.push({
        taskListId: list.id,
        taskListName: list.title,
        tasks: todayTasks
      });
    }
  });

  return result;
}

function getTodayDashboard() {
  const today = Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd');
  const taskGroups = getTodayTasks();
  const activeSession = getActiveSession();
  const logs = getLogsByDate_(today);
  const statsByTaskKey = buildTodayTaskStats_(logs, activeSession);

  taskGroups.forEach(group => {
    group.tasks = group.tasks.map(task => {
      const taskKey = getTaskKey_(task.taskListId, task.taskId);
      const stats = statsByTaskKey[taskKey] || {
        todayDurationSeconds: 0,
        todayDurationMinutes: 0,
        appStatus: 'not_started'
      };

      return Object.assign({}, task, stats);
    });
  });

  return {
    dateText: today,
    activeSession: activeSession ? buildActiveSessionView_(activeSession) : null,
    taskGroups,
    analytics: buildAnalyticsSummary_(logs, activeSession)
  };
}

function getTodayTaskStats() {
  const today = Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd');
  const activeSession = getActiveSession();
  const logs = getLogsByDate_(today);

  return buildTodayTaskStats_(logs, activeSession);
}

function getActiveSessionView() {
  const activeSession = getActiveSession();
  return activeSession ? buildActiveSessionView_(activeSession) : null;
}

function getTodayAnalytics() {
  const today = Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd');
  const activeSession = getActiveSession();
  const logs = getLogsByDate_(today);

  return buildAnalyticsSummary_(logs, activeSession);
}

function getArchiveAnalytics(startDate, endDate) {
  const range = normalizeArchiveDateRange_(startDate, endDate);
  const logs = getAllWorkLogs_().filter(log => {
    const logDate = getEffectiveLogDate_(log);
    return logDate && logDate >= range.startDate && logDate <= range.endDate;
  });

  return buildArchiveAnalyticsSummary_(logs, range.startDate, range.endDate);
}


function completeGoogleTask(taskListId, taskId) {
  const task = Tasks.Tasks.get(taskListId, taskId);
  task.status = 'completed';
  task.completed = new Date().toISOString();
  Tasks.Tasks.update(task, taskListId, taskId);
}

function debugAllTasks() {
  const taskLists = fetchAllTaskLists_();

  taskLists.forEach(list => {
    Logger.log('--- List: %s / %s ---', list.title, list.id);

    const tasks = fetchAllTasksInList_(list.id, {
      showCompleted: false,
      showHidden: true
    });

    tasks.forEach(task => {
      Logger.log(
        'title=%s | status=%s | due=%s | id=%s',
        task.title,
        task.status,
        task.due,
        task.id
      );
    });
  });
}

function setTaskDueToday(taskListId, taskId) {
  const today = Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd');

  const updatedTask = Tasks.Tasks.patch(
    {
      due: today + 'T00:00:00.000Z'
    },
    taskListId,
    taskId
  );

  return {
    taskListId,
    taskId,
    title: updatedTask.title || '',
    due: updatedTask.due || null,
    status: updatedTask.status || ''
  };
}

function getTaskCandidates() {
  const today = Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd');
  const result = [];
  const taskLists = fetchAllTaskLists_();

  taskLists.forEach(list => {
    const tasks = fetchAllTasksInList_(list.id, {
      showCompleted: false,
      showHidden: true,
      showDeleted: false
    });

    const candidates = tasks
      .filter(task => task.status !== 'completed')
      .filter(task => {
        if (!task.due) return true;
        return String(task.due).slice(0, 10) !== today;
      })
      .map(task => ({
        taskListId: list.id,
        taskListName: list.title,
        taskId: task.id,
        title: task.title || '',
        notes: task.notes || '',
        due: task.due || null,
        status: task.status
      }));

    if (candidates.length > 0) {
      result.push({
        taskListId: list.id,
        taskListName: list.title,
        tasks: candidates
      });
    }
  });

  return result;
}

function buildTodayTaskStats_(logs, activeSession) {
  const statsByTaskKey = {};

  logs.forEach(log => {
    const taskKey = getTaskKey_(log.taskListId, log.taskId);
    if (!statsByTaskKey[taskKey]) {
      statsByTaskKey[taskKey] = {
        todayDurationSeconds: 0,
        todayDurationMinutes: 0,
        appStatus: 'not_started'
      };
    }

    statsByTaskKey[taskKey].todayDurationSeconds += Number(log.durationSeconds || 0);
    statsByTaskKey[taskKey].todayDurationMinutes = Math.round(
      statsByTaskKey[taskKey].todayDurationSeconds / 60
    );
    applyLatestTaskStatus_(statsByTaskKey[taskKey], log);
  });

  if (activeSession) {
    const activeKey = getTaskKey_(activeSession.taskListId, activeSession.taskId);
    const activeDurationSeconds = getActiveSessionElapsedSeconds_(activeSession);

    if (!statsByTaskKey[activeKey]) {
      statsByTaskKey[activeKey] = {
        todayDurationSeconds: 0,
        todayDurationMinutes: 0,
        appStatus: 'running'
      };
    }

    statsByTaskKey[activeKey].todayDurationSeconds += activeDurationSeconds;
    statsByTaskKey[activeKey].todayDurationMinutes = Math.round(
      statsByTaskKey[activeKey].todayDurationSeconds / 60
    );
    statsByTaskKey[activeKey].appStatus = 'running';
  }

  return statsByTaskKey;
}

function buildActiveSessionView_(session) {
  const elapsedSeconds = getActiveSessionElapsedSeconds_(session);

  return {
    taskListId: session.taskListId,
    taskListName: session.taskListName,
    taskId: session.taskId,
    taskTitle: session.taskTitle,
    startAt: new Date(session.startAt).toISOString(),
    elapsedSeconds
  };
}

function buildAnalyticsSummary_(logs, activeSession) {
  const byTaskList = {};
  const byTask = {};
  let totalSeconds = 0;

  logs.forEach(log => {
    const seconds = Number(log.durationSeconds || 0) || (Number(log.durationMinutes || 0) * 60);
    const minutes = Number(log.durationMinutes || 0);
    const taskListKey = log.taskListId || log.taskListName;
    const taskKey = getTaskKey_(log.taskListId, log.taskId);

    totalSeconds += seconds;

    if (!byTaskList[taskListKey]) {
      byTaskList[taskListKey] = {
        taskListId: log.taskListId,
        taskListName: log.taskListName,
        totalSeconds: 0,
        totalMinutes: 0
      };
    }
    byTaskList[taskListKey].totalSeconds += seconds;
    byTaskList[taskListKey].totalMinutes += minutes;

    if (!byTask[taskKey]) {
      byTask[taskKey] = {
        taskId: log.taskId,
        taskTitle: log.taskTitle,
        taskListName: log.taskListName,
        totalSeconds: 0,
        totalMinutes: 0,
        sessionCount: 0
      };
    }
    byTask[taskKey].totalSeconds += seconds;
    byTask[taskKey].totalMinutes += minutes;
    byTask[taskKey].sessionCount += 1;
  });

  let runningSeconds = 0;
  if (activeSession) {
    runningSeconds = getActiveSessionElapsedSeconds_(activeSession);
    totalSeconds += runningSeconds;
  }

  const byTaskListItems = Object.keys(byTaskList).map(key => {
    const item = byTaskList[key];
    item.totalMinutes = Math.round(item.totalSeconds / 60);
    return item;
  });

  const byTaskItems = Object.keys(byTask).map(key => {
    const item = byTask[key];
    item.totalMinutes = Math.round(item.totalSeconds / 60);
    return item;
  });

  return {
    totalSeconds,
    totalMinutes: Math.round(totalSeconds / 60),
    totalSessions: logs.length,
    runningSeconds,
    byTaskList: byTaskListItems,
    byTask: byTaskItems
  };
}

function buildArchiveAnalyticsSummary_(logs, startDate, endDate) {
  const byDate = {};
  const byTaskList = {};
  const byTask = {};
  let totalSeconds = 0;
  let completedSessions = 0;

  logs.forEach(log => {
    const seconds = Number(log.durationSeconds || 0) || (Number(log.durationMinutes || 0) * 60);
    const taskListKey = log.taskListId || log.taskListName || 'unknown';
    const taskKey = getTaskKey_(log.taskListId || log.taskListName || 'unknown', log.taskId || log.taskTitle || 'unknown');
    const logDate = getEffectiveLogDate_(log);

    totalSeconds += seconds;
    if (log.actionType === 'completed') {
      completedSessions += 1;
    }

    if (!byDate[logDate]) {
      byDate[logDate] = {
        date: logDate,
        totalSeconds: 0,
        totalMinutes: 0,
        sessionCount: 0
      };
    }
    byDate[logDate].totalSeconds += seconds;
    byDate[logDate].sessionCount += 1;

    if (!byTaskList[taskListKey]) {
      byTaskList[taskListKey] = {
        taskListId: log.taskListId,
        taskListName: log.taskListName || '未分類',
        totalSeconds: 0,
        totalMinutes: 0,
        sessionCount: 0,
        completedCount: 0
      };
    }
    byTaskList[taskListKey].totalSeconds += seconds;
    byTaskList[taskListKey].sessionCount += 1;
    if (log.actionType === 'completed') {
      byTaskList[taskListKey].completedCount += 1;
    }

    if (!byTask[taskKey]) {
      byTask[taskKey] = {
        taskListId: log.taskListId,
        taskListName: log.taskListName || '未分類',
        taskId: log.taskId,
        taskTitle: log.taskTitle || '(no title)',
        totalSeconds: 0,
        totalMinutes: 0,
        sessionCount: 0,
        completedCount: 0,
        lastWorkedDate: logDate
      };
    }
    byTask[taskKey].totalSeconds += seconds;
    byTask[taskKey].sessionCount += 1;
    byTask[taskKey].lastWorkedDate = logDate;
    if (log.actionType === 'completed') {
      byTask[taskKey].completedCount += 1;
    }
  });

  const byDateItems = buildDateRangeItems_(startDate, endDate, byDate);
  const byTaskListItems = Object.keys(byTaskList)
    .map(key => finalizeArchiveItem_(byTaskList[key]))
    .sort(compareArchiveItemsByDuration_);
  const byTaskItems = Object.keys(byTask)
    .map(key => finalizeArchiveItem_(byTask[key]))
    .sort(compareArchiveItemsByDuration_);

  return {
    startDate,
    endDate,
    totalSeconds,
    totalMinutes: Math.round(totalSeconds / 60),
    totalSessions: logs.length,
    completedSessions,
    activeDays: byDateItems.filter(item => item.totalSeconds > 0).length,
    averageMinutesPerDay: byDateItems.length === 0 ? 0 : Math.round((totalSeconds / 60) / byDateItems.length),
    byDate: byDateItems,
    byTaskList: byTaskListItems,
    byTask: byTaskItems
  };
}

function normalizeArchiveDateRange_(startDate, endDate) {
  const todayText = Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd');
  const normalizedEnd = normalizeDateText_(endDate, new Date()) || todayText;
  const normalizedStart = normalizeDateText_(startDate, parseDateValue_(normalizedEnd)) || normalizedEnd;

  if (normalizedStart <= normalizedEnd) {
    return {
      startDate: normalizedStart,
      endDate: normalizedEnd
    };
  }

  return {
    startDate: normalizedEnd,
    endDate: normalizedStart
  };
}

function buildDateRangeItems_(startDate, endDate, byDate) {
  const items = [];
  const cursor = parseDateValue_(startDate);
  const end = parseDateValue_(endDate);

  if (!cursor || !end) return items;

  while (cursor <= end) {
    const dateText = Utilities.formatDate(cursor, TIMEZONE, 'yyyy-MM-dd');
    const item = byDate[dateText] || {
      date: dateText,
      totalSeconds: 0,
      totalMinutes: 0,
      sessionCount: 0
    };

    item.totalMinutes = Math.round(Number(item.totalSeconds || 0) / 60);
    items.push(item);
    cursor.setDate(cursor.getDate() + 1);
  }

  return items;
}

function finalizeArchiveItem_(item) {
  item.totalMinutes = Math.round(Number(item.totalSeconds || 0) / 60);
  return item;
}

function compareArchiveItemsByDuration_(a, b) {
  const secondsDiff = Number(b.totalSeconds || 0) - Number(a.totalSeconds || 0);
  if (secondsDiff !== 0) return secondsDiff;
  return String(a.taskTitle || a.taskListName || '').localeCompare(String(b.taskTitle || b.taskListName || ''));
}

function getTaskKey_(taskListId, taskId) {
  return `${taskListId}::${taskId}`;
}

function getActiveSessionElapsedSeconds_(session) {
  return Math.max(
    0,
    Math.round((new Date() - new Date(session.startAt)) / 1000)
  );
}

function applyLatestTaskStatus_(taskStats, log) {
  const logTime = getLogSortTime_(log);
  if (taskStats.lastActionAt && logTime <= taskStats.lastActionAt) {
    return;
  }

  taskStats.lastActionAt = logTime;
  taskStats.appStatus = log.actionType === 'completed' ? 'completed' : 'paused';
}

function getLogSortTime_(log) {
  const endTime = new Date(log.endTime || log.startTime || 0).getTime();
  if (!Number.isNaN(endTime) && endTime > 0) return endTime;

  const startTime = new Date(log.startTime || 0).getTime();
  if (!Number.isNaN(startTime) && startTime > 0) return startTime;

  return 0;
}

function fetchAllTaskLists_() {
  const items = [];
  let pageToken = null;

  do {
    const response = Tasks.Tasklists.list({
      maxResults: 100,
      pageToken
    });

    items.push.apply(items, response.items || []);
    pageToken = response.nextPageToken || null;
  } while (pageToken);

  return items;
}

function fetchAllTasksInList_(taskListId, options) {
  const items = [];
  let pageToken = null;

  do {
    const response = Tasks.Tasks.list(taskListId, Object.assign({}, options, {
      maxResults: 100,
      pageToken
    }));

    items.push.apply(items, response.items || []);
    pageToken = response.nextPageToken || null;
  } while (pageToken);

  return items;
}
