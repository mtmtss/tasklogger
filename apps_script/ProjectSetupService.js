function initializeTaskLoggerProject() {
  const properties = PropertiesService.getScriptProperties();
  let spreadsheetId = properties.getProperty('spreadsheetId');
  let spreadsheet;
  let createdSpreadsheet = false;

  if (spreadsheetId) {
    spreadsheet = SpreadsheetApp.openById(spreadsheetId);
  } else {
    spreadsheet = SpreadsheetApp.create('TaskLogger_GAS_Data');
    spreadsheetId = spreadsheet.getId();
    properties.setProperty('spreadsheetId', spreadsheetId);
    createdSpreadsheet = true;
  }

  ensureProjectSheets_(spreadsheet);
  ensureProjectSettings_(spreadsheet.getSheetByName('settings'));

  return {
    spreadsheetId,
    spreadsheetUrl: spreadsheet.getUrl(),
    createdSpreadsheet
  };
}

function connectTaskLoggerSpreadsheet(spreadsheetId) {
  if (!spreadsheetId) {
    throw new Error('spreadsheetId is required.');
  }

  const spreadsheet = SpreadsheetApp.openById(spreadsheetId);
  PropertiesService.getScriptProperties().setProperty('spreadsheetId', spreadsheet.getId());
  ensureProjectSheets_(spreadsheet);
  ensureProjectSettings_(spreadsheet.getSheetByName('settings'));

  return {
    spreadsheetId: spreadsheet.getId(),
    spreadsheetUrl: spreadsheet.getUrl(),
    createdSpreadsheet: false
  };
}

function getTaskLoggerSetupStatus() {
  const spreadsheetId = PropertiesService.getScriptProperties().getProperty('spreadsheetId');
  if (!spreadsheetId) {
    return {
      initialized: false,
      spreadsheetId: '',
      spreadsheetUrl: ''
    };
  }

  const spreadsheet = SpreadsheetApp.openById(spreadsheetId);
  return {
    initialized: true,
    spreadsheetId,
    spreadsheetUrl: spreadsheet.getUrl()
  };
}

function ensureProjectSheets_(spreadsheet) {
  const requiredSheets = ['WorkLogs', 'ActiveSession', 'settings'];
  requiredSheets.forEach(name => {
    if (!spreadsheet.getSheetByName(name)) {
      spreadsheet.insertSheet(name);
    }
  });

  ensureWorkLogSheetSchema_(spreadsheet.getSheetByName('WorkLogs'));
  ensureActiveSessionSheet_(spreadsheet.getSheetByName('ActiveSession'));
}

function ensureActiveSessionSheet_(sheet) {
  const expected = [
    'taskListId',
    'taskListName',
    'taskId',
    'taskTitle',
    'startAt'
  ];

  if (sheet.getLastRow() === 0) {
    sheet.appendRow(expected);
    return;
  }

  const headers = sheet.getRange(1, 1, 1, Math.max(1, sheet.getLastColumn())).getValues()[0];
  const isExact = expected.every((value, index) => headers[index] === value);

  if (!isExact) {
    sheet.clearContents();
    sheet.appendRow(expected);
  }
}

function ensureProjectSettings_(sheet) {
  const defaults = [
    ['key', 'value'],
    ['timezone', TIMEZONE],
    ['allowMultipleRunningTasks', 'false'],
    ['morningReportTaskLimit', '15'],
    ['geminiIncludeTaskNotes', 'false'],
    ['geminiModel', 'gemini-2.5-flash'],
    ['reportEmail', ''],
    ['geminiApiKey', '']
  ];

  if (sheet.getLastRow() === 0) {
    sheet.getRange(1, 1, defaults.length, 2).setValues(defaults);
    return;
  }

  const values = sheet.getDataRange().getValues();
  const existingKeys = {};
  values.slice(1).forEach(row => {
    const key = String(row[0] || '').trim();
    if (key) existingKeys[key] = true;
  });

  if (values[0][0] !== 'key' || values[0][1] !== 'value') {
    sheet.clearContents();
    sheet.getRange(1, 1, defaults.length, 2).setValues(defaults);
    return;
  }

  const missingRows = defaults.slice(1).filter(row => !existingKeys[row[0]]);
  if (missingRows.length > 0) {
    sheet.getRange(sheet.getLastRow() + 1, 1, missingRows.length, 2).setValues(missingRows);
  }
}
