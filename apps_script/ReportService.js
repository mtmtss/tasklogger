function sendMorningReport() {
  const email = getSetting_('reportEmail', '');
  if (!email) {
    throw new Error('reportEmail を Script Properties か settings シートに設定してください。');
  }

  const report = buildMorningReport_();

  GmailApp.sendEmail(
    email,
    report.subject,
    report.body
  );
}

function buildMorningReport_() {
  const yesterday = new Date();
  yesterday.setDate(yesterday.getDate() - 1);

  const dateText = Utilities.formatDate(yesterday, TIMEZONE, 'yyyy-MM-dd');
  const yesterdayLogs = getLogsByDate_(dateText);
  const recentLogs = getRecentWorkLogs_(7);
  const openTasks = getOpenTasksForMorningReport_(recentLogs);
  const taskInsights = buildTaskInsights_(openTasks, recentLogs);

  const sections = [
    `朝のレポート ${Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd')}`,
    '',
    buildYesterdaySummarySection_(dateText, yesterdayLogs),
    buildOpenTasksSection_(openTasks),
    buildRecommendationSection_(taskInsights)
  ];

  return {
    subject: '朝の作業レポート',
    body: sections.join('\n').replace(/\n{3,}/g, '\n\n')
  };
}

function buildYesterdaySummarySection_(dateText, logs) {
  if (logs.length === 0) {
    return [
      '【昨日の実績】',
      `${dateText} の作業ログはありませんでした。`
    ].join('\n');
  }

  const byTaskList = {};
  let totalMinutes = 0;
  let completedCount = 0;

  logs.forEach(log => {
    const taskListName = log.taskListName || '未分類';
    const durationMinutes = Number(log.durationMinutes || 0);

    totalMinutes += durationMinutes;
    if (log.actionType === 'completed') {
      completedCount += 1;
    }

    if (!byTaskList[taskListName]) {
      byTaskList[taskListName] = [];
    }

    byTaskList[taskListName].push(log);
  });

  const lines = [
    '【昨日の実績】',
    `${dateText} / 合計 ${totalMinutes}分 / 完了 ${completedCount}件`
  ];

  Object.keys(byTaskList).forEach(taskListName => {
    const items = byTaskList[taskListName];
    const subtotal = items.reduce((sum, item) => sum + Number(item.durationMinutes || 0), 0);

    lines.push(`- ${taskListName}: ${subtotal}分`);

    items.slice(0, 5).forEach(item => {
      const statusLabel = item.actionType === 'completed' ? '完了' : '中断';
      lines.push(`  ${item.taskTitle} (${item.durationMinutes}分, ${statusLabel})`);
    });

    if (items.length > 5) {
      lines.push(`  ...ほか ${items.length - 5}件`);
    }
  });

  return lines.join('\n');
}

function buildOpenTasksSection_(tasks) {
  const limit = Math.min(tasks.length, 10);
  const lines = [
    `【現在の未完了タスク】 ${tasks.length}件`
  ];

  if (tasks.length === 0) {
    lines.push('未完了タスクはありません。');
    return lines.join('\n');
  }

  for (let i = 0; i < limit; i += 1) {
    const task = tasks[i];
    const dueLabel = task.dueStatusLabel ? ` / ${task.dueStatusLabel}` : '';
    const workedLabel = task.recentWorkedMinutes > 0 ? ` / 直近作業 ${task.recentWorkedMinutes}分` : '';
    lines.push(`- ${task.taskListName} | ${task.title}${dueLabel}${workedLabel}`);
  }

  if (tasks.length > limit) {
    lines.push(`- ...ほか ${tasks.length - limit}件`);
  }

  return lines.join('\n');
}

function buildRecommendationSection_(taskInsights) {
  const lines = [
    '【着手おすすめ】',
    `最優先: ${taskInsights.topTaskTitle || '候補なし'}`,
    taskInsights.summary || '提案を生成できませんでした。'
  ];

  if (taskInsights.recommendedSteps && taskInsights.recommendedSteps.length > 0) {
    lines.push('');
    lines.push('今日の進め方');
    taskInsights.recommendedSteps.forEach(step => {
      lines.push(`- ${step}`);
    });
  }

  if (taskInsights.sourceLabel) {
    lines.push('');
    lines.push(`判定ソース: ${taskInsights.sourceLabel}`);
  }

  return lines.join('\n');
}

function buildTaskInsights_(tasks, recentLogs) {
  if (tasks.length === 0) {
    return {
      topTaskTitle: '',
      summary: '今日は新しく着手するタスクが見当たりません。昨日の振り返りと整理から始めるのがおすすめです。',
      recommendedSteps: [
        'Google Tasks を見直して今日やるものを1件追加する',
        '昨日の中断タスクに再開価値があるか確認する'
      ],
      sourceLabel: 'rule-based fallback'
    };
  }

  const geminiApiKey = getSetting_('geminiApiKey', '');
  if (!geminiApiKey) {
    return buildFallbackTaskInsights_(tasks);
  }

  try {
    return requestGeminiTaskInsights_(tasks, recentLogs, geminiApiKey);
  } catch (error) {
    const fallback = buildFallbackTaskInsights_(tasks);
    fallback.summary += `\nGemini呼び出しに失敗したため、ルールベースで提案しています。(${error.message})`;
    return fallback;
  }
}

function buildFallbackTaskInsights_(tasks) {
  const sorted = tasks.slice().sort(compareMorningTaskPriority_);
  const topTask = sorted[0];
  const reasons = [];

  if (topTask.isOverdue) {
    reasons.push('期限を過ぎているため');
  } else if (topTask.isDueToday) {
    reasons.push('今日が期限のため');
  } else if (topTask.recentWorkedMinutes > 0) {
    reasons.push('すでに文脈が温まっていて再開コストが低いため');
  } else {
    reasons.push('現在の未完了タスクの中で優先度がもっとも高いため');
  }

  return {
    topTaskTitle: `${topTask.taskListName} / ${topTask.title}`,
    summary: `${topTask.title} から始めるのがおすすめです。${reasons.join('、')}です。`,
    recommendedSteps: [
      `${topTask.title} にまず30分集中する`,
      '終わりに次の1アクションをメモして中断コストを下げる'
    ],
    sourceLabel: 'rule-based fallback'
  };
}

function requestGeminiTaskInsights_(tasks, recentLogs, apiKey) {
  const model = getSetting_('geminiModel', 'gemini-2.5-flash');
  const endpoint = `https://generativelanguage.googleapis.com/v1beta/models/${encodeURIComponent(model)}:generateContent?key=${encodeURIComponent(apiKey)}`;
  const prompt = buildGeminiPrompt_(tasks, recentLogs);
  const response = UrlFetchApp.fetch(endpoint, {
    method: 'post',
    contentType: 'application/json',
    muteHttpExceptions: true,
    payload: JSON.stringify({
      contents: [
        {
          role: 'user',
          parts: [
            { text: prompt }
          ]
        }
      ],
      generationConfig: {
        temperature: 0.4,
        responseMimeType: 'application/json'
      }
    })
  });

  const statusCode = response.getResponseCode();
  const bodyText = response.getContentText();
  if (statusCode < 200 || statusCode >= 300) {
    throw new Error(`HTTP ${statusCode}: ${bodyText}`);
  }

  const payload = JSON.parse(bodyText);
  const rawText = extractGeminiText_(payload);
  if (!rawText) {
    throw new Error('Gemini response text was empty.');
  }

  const parsed = parseGeminiJson_(rawText);
  return {
    topTaskTitle: parsed.topTaskTitle || '',
    summary: parsed.summary || '',
    recommendedSteps: Array.isArray(parsed.recommendedSteps) ? parsed.recommendedSteps.slice(0, 3) : [],
    sourceLabel: `Gemini (${model})`
  };
}

function buildGeminiPrompt_(tasks, recentLogs) {
  const taskLimit = Number(getSetting_('morningReportTaskLimit', 15)) || 15;
  const includeTaskNotes = getBooleanSetting_('geminiIncludeTaskNotes', false);
  const limitedTasks = tasks.slice(0, Math.max(1, taskLimit));
  const recentTaskStats = buildRecentTaskStatsForPrompt_(recentLogs);
  const promptPayload = {
    today: Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd'),
    instructions: [
      'あなたは朝の作業開始を支援するアシスタントです。',
      '未完了タスク一覧と直近の作業ログを見て、最初に着手すべきタスクを1つ選んでください。',
      '締切の近さ、期限超過、直近の進捗、再開しやすさ、集中のしやすさを考慮してください。',
      'JSONのみで返してください。'
    ],
    outputSchema: {
      topTaskTitle: 'task list and title in one short string',
      summary: '2-4 sentences in Japanese',
      recommendedSteps: ['short step 1', 'short step 2', 'short step 3']
    },
    openTasks: limitedTasks.map(task => ({
      taskListName: task.taskListName,
      title: task.title,
      due: task.due || '',
      dueStatusLabel: task.dueStatusLabel,
      recentWorkedMinutes: task.recentWorkedMinutes,
      recentSessionCount: task.recentSessionCount,
      notes: includeTaskNotes ? (task.notes || '') : ''
    })),
    recentTaskStats
  };

  return JSON.stringify(promptPayload, null, 2);
}

function extractGeminiText_(payload) {
  if (!payload || !payload.candidates || payload.candidates.length === 0) {
    return '';
  }

  const parts = (((payload.candidates[0] || {}).content || {}).parts || []);
  return parts
    .map(part => part.text || '')
    .join('')
    .trim();
}

function parseGeminiJson_(rawText) {
  const normalized = rawText
    .replace(/^```json\s*/i, '')
    .replace(/^```\s*/i, '')
    .replace(/\s*```$/i, '')
    .trim();

  return JSON.parse(normalized);
}

function buildRecentTaskStatsForPrompt_(logs) {
  const stats = {};

  logs.forEach(log => {
    const taskKey = getTaskKey_(log.taskListId, log.taskId);
    if (!stats[taskKey]) {
      stats[taskKey] = {
        taskListName: log.taskListName,
        taskTitle: log.taskTitle,
        totalMinutes: 0,
        sessions: 0,
        lastActionType: log.actionType,
        lastLogDate: log.logDate
      };
    }

    stats[taskKey].totalMinutes += Number(log.durationMinutes || 0);
    stats[taskKey].sessions += 1;
    stats[taskKey].lastActionType = log.actionType;
    stats[taskKey].lastLogDate = log.logDate;
  });

  return Object.keys(stats).map(key => stats[key]);
}

function getRecentWorkLogs_(days) {
  const end = new Date();
  const start = new Date();
  start.setDate(start.getDate() - Math.max(1, days) + 1);

  return getAllWorkLogs_().filter(log => {
    const startTime = parseDateValue_(log.startTime);
    if (!startTime) return false;
    return startTime >= start && startTime <= end;
  });
}

function getOpenTasksForMorningReport_(recentLogs) {
  const taskLists = fetchAllTaskLists_();
  const recentStatsByTaskKey = buildRecentTaskStatsMap_(recentLogs);
  const tasks = [];

  taskLists.forEach(list => {
    const items = fetchAllTasksInList_(list.id, {
      showCompleted: false,
      showHidden: true,
      showDeleted: false
    });

    items
      .filter(task => task.status !== 'completed')
      .forEach(task => {
        const taskKey = getTaskKey_(list.id, task.id);
        const recentStats = recentStatsByTaskKey[taskKey] || {
          recentWorkedMinutes: 0,
          recentSessionCount: 0
        };

        tasks.push({
          taskListId: list.id,
          taskListName: list.title || 'Untitled',
          taskId: task.id,
          title: task.title || '(no title)',
          notes: task.notes || '',
          due: task.due || '',
          dueStatusLabel: buildDueStatusLabel_(task.due),
          isOverdue: isTaskOverdue_(task.due),
          isDueToday: isTaskDueToday_(task.due),
          recentWorkedMinutes: recentStats.recentWorkedMinutes,
          recentSessionCount: recentStats.recentSessionCount
        });
      });
  });

  return tasks.sort(compareMorningTaskPriority_);
}

function buildRecentTaskStatsMap_(logs) {
  const map = {};

  logs.forEach(log => {
    const taskKey = getTaskKey_(log.taskListId, log.taskId);
    if (!map[taskKey]) {
      map[taskKey] = {
        recentWorkedMinutes: 0,
        recentSessionCount: 0
      };
    }

    map[taskKey].recentWorkedMinutes += Number(log.durationMinutes || 0);
    map[taskKey].recentSessionCount += 1;
  });

  return map;
}

function compareMorningTaskPriority_(a, b) {
  const scoreA = getMorningTaskPriorityScore_(a);
  const scoreB = getMorningTaskPriorityScore_(b);

  if (scoreA !== scoreB) {
    return scoreB - scoreA;
  }

  return String(a.title || '').localeCompare(String(b.title || ''));
}

function getMorningTaskPriorityScore_(task) {
  let score = 0;

  if (task.isOverdue) score += 1000;
  if (task.isDueToday) score += 600;
  if (task.recentWorkedMinutes > 0) score += Math.min(240, task.recentWorkedMinutes);
  if (task.recentSessionCount > 0) score += Math.min(80, task.recentSessionCount * 10);
  if (!task.due) score += 20;

  return score;
}

function isTaskDueToday_(due) {
  if (!due) return false;
  const dueDate = String(due).slice(0, 10);
  const today = Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd');
  return dueDate === today;
}

function isTaskOverdue_(due) {
  if (!due) return false;
  const dueDate = String(due).slice(0, 10);
  const today = Utilities.formatDate(new Date(), TIMEZONE, 'yyyy-MM-dd');
  return dueDate < today;
}

function buildDueStatusLabel_(due) {
  if (!due) return '期限なし';

  const dueDate = String(due).slice(0, 10);
  if (isTaskOverdue_(due)) return `期限超過 ${dueDate}`;
  if (isTaskDueToday_(due)) return `今日期限 ${dueDate}`;
  return `期限 ${dueDate}`;
}

function getBooleanSetting_(key, defaultValue) {
  const fallback = defaultValue ? 'true' : 'false';
  const value = String(getSetting_(key, fallback)).trim().toLowerCase();
  return value === 'true' || value === '1' || value === 'yes' || value === 'on';
}
