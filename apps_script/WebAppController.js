const TIMEZONE = 'Asia/Tokyo';
const APPLE_TOUCH_ICON_URL = 'https://477848df.apple-touch-icon-903.pages.dev/timer-icon-180.png';
let settingsCache_ = null;

function getSetting_(key, defaultValue) {
  const scriptValue = PropertiesService.getScriptProperties().getProperty(key);
  if (scriptValue !== null && scriptValue !== '') {
    return scriptValue;
  }

  if (!settingsCache_) {
    settingsCache_ = loadSettingsCache_();
  }

  if (Object.prototype.hasOwnProperty.call(settingsCache_, key)) {
    return settingsCache_[key];
  }

  return defaultValue || '';
}

function loadSettingsCache_() {
  const sheet = getSheet_('settings');
  const values = sheet.getDataRange().getValues();
  const map = {};

  for (let i = 1; i < values.length; i += 1) {
    const rowKey = String(values[i][0] || '').trim();
    if (!rowKey) continue;
    map[rowKey] = String(values[i][1] || '').trim();
  }

  return map;
}

function doGet(e) {
  const page = (e && e.parameter && e.parameter.page) || 'today';
  const templateName = page === 'archive' ? 'ArchivePage' : 'TodayPage';
  const template = HtmlService.createTemplateFromFile(templateName);

  template.APP_BASE_URL = getAppBaseUrl_();
  template.APPLE_TOUCH_ICON_URL = APPLE_TOUCH_ICON_URL;

  return template
    .evaluate()
    .setTitle(page === 'archive' ? 'Task Archive' : 'Today Tasks')
    .addMetaTag('viewport', 'width=device-width, initial-scale=1');
}

function getTodayTasksData() {
  return getTodayTasks();
}

function getTodayDashboardPageData() {
  return {
    dashboard: getTodayDashboard(),
    candidates: getTaskCandidates()
  };
}

function getTodayDashboardData() {
  return getTodayDashboard();
}

function getTodayTaskStatsData() {
  return getTodayTaskStats();
}

function getActiveSessionData() {
  return getActiveSessionView();
}

function getTodayAnalyticsData() {
  return getTodayAnalytics();
}

function getTaskCandidatesData() {
  return getTaskCandidates();
}

function scheduleTaskForToday(taskListId, taskId) {
  return setTaskDueToday(taskListId, taskId);
}

function getArchiveAnalyticsData(startDate, endDate) {
  return getArchiveAnalytics(startDate, endDate);
}

function getArchiveDashboardPageData() {
  const today = Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd');
  const defaultStartDate = getRelativeDateText_(-29);

  return {
    archive: getArchiveAnalytics(defaultStartDate, today)
  };
}

function getRelativeDateText_(offsetDays) {
  const date = new Date();
  date.setDate(date.getDate() + Number(offsetDays || 0));
  return Utilities.formatDate(date, TIMEZONE, 'yyyy-MM-dd');
}

function getAppBaseUrl_() {
  return ScriptApp.getService().getUrl() || '';
}

function getSpreadsheetId_() {
  const spreadsheetId = PropertiesService.getScriptProperties().getProperty('spreadsheetId');
  if (!spreadsheetId) {
    throw new Error(
      'spreadsheetId is not configured. Run initializeTaskLoggerProject() or set spreadsheetId in Script Properties.'
    );
  }

  return spreadsheetId;
}
