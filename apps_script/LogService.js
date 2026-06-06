function startTask(task) {
  const activeSession = getActiveSession();
  if (activeSession) {
    const isSameTask = activeSession.taskListId === task.taskListId &&
      activeSession.taskId === task.taskId;

    if (!isSameTask) {
      throw new Error('現在作業中のタスクがあります。先に中断または完了してください。');
    }

    return activeSession;
  }

  const sheet = getSheet_('ActiveSession');
  sheet.clearContents();

  sheet.appendRow([
    'taskListId',
    'taskListName',
    'taskId',
    'taskTitle',
    'startAt'
  ]);

  sheet.appendRow([
    task.taskListId,
    task.taskListName,
    task.taskId,
    task.title,
    new Date()
  ]);

  return getActiveSession();
}

function getActiveSession() {
  const sheet = getSheet_('ActiveSession');
  const values = sheet.getDataRange().getValues();

  if (values.length < 2) return null;

  const row = values[1];

  return {
    taskListId: row[0],
    taskListName: row[1],
    taskId: row[2],
    taskTitle: row[3],
    startAt: row[4]
  };
}

function stopTask(action, memo) {
  const session = getActiveSession();
  if (!session) {
    throw new Error('作業中のタスクがありません。');
  }

  if (!session.startAt) {
    throw new Error('開始時刻が見つからないため、作業時間を記録できません。');
  }

  const actionType = normalizeActionType_(action);

  const endAt = new Date();
  const startAt = new Date(session.startAt);
  const durationSeconds = Math.max(0, Math.floor((endAt - startAt) / 1000));
  const durationMinutes = durationSeconds === 0 ? 0 : Math.ceil(durationSeconds / 60);

  appendWorkLog({
    logDate: Utilities.formatDate(startAt, TIMEZONE, 'yyyy-MM-dd'),
    userId: Session.getActiveUser().getEmail() || '',
    taskListId: session.taskListId,
    taskListName: session.taskListName,
    taskId: session.taskId,
    taskTitle: session.taskTitle,
    startTime: startAt,
    endTime: endAt,
    durationSeconds,
    durationMinutes,
    actionType,
    memo: memo || ''
  });

  if (actionType === 'completed') {
    completeGoogleTask(session.taskListId, session.taskId);
  }

  getSheet_('ActiveSession').clearContents();

  return {
    durationSeconds,
    durationMinutes,
    actionType
  };
}

function appendWorkLog(log) {
  const sheet = getSheet_('WorkLogs');
  const logId = Utilities.getUuid();
  const createdAt = new Date();
  const schema = ensureWorkLogSheetSchema_(sheet);

  if (schema === 'legacy') {
    sheet.appendRow([
      logId,
      log.logDate,
      log.taskListId,
      log.taskListName,
      log.taskId,
      log.taskTitle,
      log.startTime,
      log.endTime,
      log.durationMinutes,
      log.actionType,
      log.memo,
      createdAt
    ]);
    return;
  }

  sheet.appendRow([
    logId,
    log.userId || '',
    log.taskListId,
    log.taskListName,
    log.taskId,
    log.taskTitle,
    log.actionType,
    log.startTime.toISOString(),
    log.endTime.toISOString(),
    log.durationSeconds,
    log.durationMinutes,
    log.logDate,
    log.memo,
    createdAt.toISOString()
  ]);
}

function getSheet_(name) {
  const spreadsheet = SpreadsheetApp.openById(getSpreadsheetId_());
  let sheet = spreadsheet.getSheetByName(name);

  if (!sheet) {
    sheet = spreadsheet.insertSheet(name);
  }

  return sheet;
}

function getLogsByDate_(dateText) {
  return getAllWorkLogs_()
    .filter(log => getEffectiveLogDate_(log) === dateText);
}

function getAllWorkLogs_() {
  const sheet = getSheet_('WorkLogs');
  const values = sheet.getDataRange().getValues();

  if (values.length < 2) return [];

  const headers = values[0];
  const rows = values.slice(1);

  return rows
    .filter(row => row.some(cell => cell !== ''))
    .map(row => normalizeLogRow_(headers, row));
}

function normalizeLogRow_(headers, row) {
  const record = {};
  headers.forEach((header, index) => {
    record[header] = row[index];
  });

  const actionType = normalizeActionType_(record.actionType || record.action || '');
  const startTime = record.startTime || record.startAt || '';
  const endTime = record.endTime || record.endAt || '';
  const durationSeconds = Number(record.durationSeconds || 0);
  const durationMinutes = Number(record.durationMinutes || record.minutes || 0);
  const logDate = record.logDate || record.date || '';
  const normalizedStartTime = parseDateValue_(startTime);
  const normalizedEndTime = parseDateValue_(endTime);
  const normalizedLogDate = normalizeDateText_(logDate, normalizedStartTime);
  const resolvedDurationSeconds = resolveDurationSeconds_(
    durationSeconds,
    durationMinutes,
    normalizedStartTime,
    normalizedEndTime
  );

  return {
    logId: record.logId || '',
    userId: record.userId || '',
    taskListId: record.taskListId || '',
    taskListName: record.taskListName || '',
    taskId: record.taskId || '',
    taskTitle: record.taskTitle || '',
    actionType,
    startTime: normalizedStartTime ? normalizedStartTime.toISOString() : startTime,
    endTime: normalizedEndTime ? normalizedEndTime.toISOString() : endTime,
    durationSeconds: resolvedDurationSeconds,
    durationMinutes: resolvedDurationSeconds === 0 ? 0 : Math.ceil(resolvedDurationSeconds / 60),
    logDate: normalizedLogDate,
    memo: record.memo || '',
    createdAt: record.createdAt || ''
  };
}

function getEffectiveLogDate_(log) {
  return normalizeDateText_(log.logDate, parseDateValue_(log.startTime));
}

function parseDateValue_(value) {
  if (!value) return null;
  if (Object.prototype.toString.call(value) === '[object Date]') return value;

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return null;
  return parsed;
}

function normalizeDateText_(value, fallbackDate) {
  const parsed = parseDateValue_(value);
  if (parsed) {
    return Utilities.formatDate(parsed, TIMEZONE, 'yyyy-MM-dd');
  }

  if (typeof value === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(value)) {
    return value;
  }

  if (fallbackDate) {
    return Utilities.formatDate(fallbackDate, TIMEZONE, 'yyyy-MM-dd');
  }

  return '';
}

function resolveDurationSeconds_(durationSeconds, durationMinutes, startTime, endTime) {
  if (durationSeconds > 0) return durationSeconds;
  if (startTime && endTime) {
    const computed = Math.floor((endTime - startTime) / 1000);
    if (computed > 0) return computed;
  }
  if (durationMinutes > 0) return durationMinutes * 60;
  return 0;
}

function normalizeActionType_(action) {
  if (action === 'completed') return 'completed';
  return 'paused';
}

function ensureWorkLogSheetSchema_(sheet) {
  if (sheet.getLastRow() === 0) {
    sheet.appendRow([
      'logId',
      'userId',
      'taskListId',
      'taskListName',
      'taskId',
      'taskTitle',
      'actionType',
      'startTime',
      'endTime',
      'durationSeconds',
      'durationMinutes',
      'logDate',
      'memo',
      'createdAt'
    ]);
    return 'current';
  }

  const headers = sheet.getRange(1, 1, 1, sheet.getLastColumn()).getValues()[0];

  if (headers.includes('actionType') && headers.includes('durationSeconds')) {
    return 'current';
  }

  return 'legacy';
}
