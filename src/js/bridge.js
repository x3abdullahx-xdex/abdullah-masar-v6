// Abdullah Masar V6 — Dual-Mode Bridge (Tauri v2 Rust SQLite + Browser Single-Instance Fallback)
(function () {
  const isTauri = Boolean(window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke);
  const STORAGE_KEY = 'masar_os_v6_browser_store';
  const CRED_KEY = 'masar_v6_sync_credentials';
  const PRAYERS = ['fajr', 'dhuhr', 'asr', 'maghrib', 'isha'];

  // Browser Single-Instance Guard (BroadcastChannel + navigator.locks fallback when outside Tauri)
  function initWebSingleInstance(onDuplicate) {
    if (isTauri) return;
    if ('BroadcastChannel' in window) {
      const ch = new BroadcastChannel('abdullah_masar_v6_instance');
      ch.postMessage({ type: 'HELLO', ts: Date.now() });
      ch.onmessage = (ev) => {
        if (ev.data && ev.data.type === 'HELLO') {
          ch.postMessage({ type: 'ALREADY_OPEN' });
        } else if (ev.data && ev.data.type === 'ALREADY_OPEN') {
          if (typeof onDuplicate === 'function') onDuplicate();
        }
      };
    }
  }

  function uuid() {
    if (crypto && crypto.randomUUID) return crypto.randomUUID();
    return 'id_' + Date.now().toString(36) + '_' + Math.random().toString(36).slice(2, 9);
  }

  function loadWebStore() {
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) return JSON.parse(raw);
    } catch (_) {}

    const seed = window.MASAR_SEED || { quotes: [], defaultSchedule: {}, defaultExtraTasks: [] };
    const now = Date.now();
    const deviceId = 'web_' + uuid().slice(0, 8);

    const recurrence_rules = [];
    Object.entries(seed.defaultSchedule || {}).forEach(([dayStr, blocks]) => {
      const weekday = Number(dayStr);
      (blocks || []).forEach((b, idx) => {
        recurrence_rules.push({
          id: b.id || uuid(),
          weekday,
          title: b.title,
          category: b.category || 'شخصي',
          start_time: b.start || '09:00',
          end_time: b.end || null,
          prayer_slot: b.prayer || null,
          note: b.note || null,
          is_work_shift: b.category === 'عمل' || String(b.category).toLowerCase() === 'work',
          sort_order: idx,
          active: true,
          updated_at: now,
          device_id: deviceId,
          deleted_at: null
        });
      });
    });

    const tasks = (seed.defaultExtraTasks || []).map((t) => ({
      id: t.id || uuid(),
      logical_date: t.date,
      original_logical_date: t.date,
      rule_id: null,
      title: t.title,
      category: t.category || 'شخصي',
      start_time: t.time || null,
      end_time: null,
      note: t.note || null,
      priority: t.priority || 'normal',
      status: t.status === 'done' ? 'done' : 'pending',
      reschedule_count: 0,
      reminder_at: t.reminderAt || null,
      reminder_fired: false,
      updated_at: now,
      device_id: deviceId,
      deleted_at: null
    }));

    const quotes = (seed.quotes || []).map((q) => ({
      id: q.id,
      category: q.cat,
      text: q.text,
      source: q.source || '',
      is_builtin: true,
      is_favorite: ['q21', 'q22', 'q1'].includes(q.id),
      updated_at: now,
      device_id: deviceId,
      deleted_at: null
    }));

    const store = {
      schema_version: 6,
      device_id: deviceId,
      meta: {
        theme: 'dark',
        calmMode: false,
        sfxMuted: false,
        pinnedQuoteId: null,
        lastSyncEtag: null
      },
      recurrence_rules,
      tasks,
      reschedule_events: [],
      cancellation_excuses: [],
      prayers_log: {}, // `${date}:${prayer}` -> { logical_date, prayer_key, completed, updated_at, device_id }
      day_summaries: {}, // `${date}` -> { logical_date, is_shift_day, frozen, shield_week_key, updated_at, device_id }
      focus_sessions: [],
      distractions: [],
      notes: [],
      quotes
    };
    saveWebStore(store);
    return store;
  }

  function saveWebStore(store) {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(store));
  }

  function materializeWebDay(store, logicalDate) {
    const weekday = window.MasarTime.getWeekday(logicalDate);
    const rules = store.recurrence_rules
      .filter((r) => r.weekday === weekday && r.active && !r.deleted_at)
      .sort((a, b) => a.sort_order - b.sort_order);

    let hasShift = false;
    rules.forEach((r) => {
      if (r.is_work_shift || r.category === 'عمل') hasShift = true;
      const exists = store.tasks.some((t) => t.rule_id === r.id && t.original_logical_date === logicalDate);
      if (!exists) {
        store.tasks.push({
          id: uuid(),
          logical_date: logicalDate,
          original_logical_date: logicalDate,
          rule_id: r.id,
          title: r.title,
          category: r.category,
          start_time: r.start_time,
          end_time: r.end_time,
          note: r.note,
          priority: 'normal',
          status: 'pending',
          reschedule_count: 0,
          reminder_at: null,
          reminder_fired: false,
          updated_at: Date.now(),
          device_id: store.device_id,
          deleted_at: null
        });
      }
    });

    if (!store.day_summaries[logicalDate]) {
      store.day_summaries[logicalDate] = {
        logical_date: logicalDate,
        is_shift_day: hasShift,
        frozen: false,
        shield_week_key: null,
        updated_at: Date.now(),
        device_id: store.device_id
      };
    } else {
      store.day_summaries[logicalDate].is_shift_day = hasShift;
    }
    saveWebStore(store);
  }

  function computeScore(tasks, prayers, focusMins, isShiftDay, frozen) {
    const prayersDone = PRAYERS.filter((p) => Boolean(prayers[p])).length;
    const active = tasks.filter((t) => !t.deleted_at && t.status !== 'rescheduled');
    const tasksTotal = active.length;
    const tasksDone = active.filter((t) => t.status === 'done').length;
    const pScore = (prayersDone / 5) * 45;
    const tRatio = tasksTotal > 0 ? tasksDone / tasksTotal : 1;
    let totalPct;
    if (isShiftDay) {
      totalPct = Math.min(100, pScore + tRatio * 55 + Math.min(1, focusMins / 90) * 10);
    } else {
      totalPct = Math.min(100, pScore + tRatio * 40 + Math.min(1, focusMins / 120) * 15);
    }
    totalPct = Math.round(totalPct * 10) / 10;
    return {
      total_pct: totalPct,
      prayers_done: prayersDone,
      tasks_done: tasksDone,
      tasks_total: tasksTotal,
      focus_mins: focusMins,
      is_shift_day: isShiftDay,
      qualified: Boolean(frozen || totalPct >= 60)
    };
  }

  const api = {
    isTauri,
    initWebSingleInstance,

    async getDayBundle(logicalDate) {
      if (isTauri) {
        return window.__TAURI__.core.invoke('get_day_bundle', { logicalDate });
      }
      const store = loadWebStore();
      materializeWebDay(store, logicalDate);
      const tasks = store.tasks
        .filter((t) => t.logical_date === logicalDate && !t.deleted_at)
        .sort((a, b) => (a.start_time || '23:59').localeCompare(b.start_time || '23:59'));
      const prayers = {};
      PRAYERS.forEach((p) => {
        const row = store.prayers_log[`${logicalDate}:${p}`];
        prayers[p] = Boolean(row && row.completed);
      });
      const summary = store.day_summaries[logicalDate];
      const focus_sessions = store.focus_sessions.filter((s) => s.logical_date === logicalDate && !s.deleted_at);
      const focusMins = focus_sessions.reduce((acc, s) => acc + (s.actual_mins || 0), 0);
      const distractions = store.distractions.filter((d) => d.logical_date === logicalDate && !d.deleted_at);
      const score = computeScore(tasks, prayers, focusMins, summary.is_shift_day, summary.frozen);
      return {
        logical_date: logicalDate,
        is_extended_night: window.MasarTime.isExtendedNight(),
        tasks,
        prayers,
        summary,
        score,
        focus_sessions,
        distractions
      };
    },

    async createTask(input) {
      if (isTauri) return window.__TAURI__.core.invoke('create_task', { input });
      const store = loadWebStore();
      const id = uuid();
      store.tasks.push({
        id,
        logical_date: input.logical_date,
        original_logical_date: input.logical_date,
        rule_id: null,
        title: input.title.trim(),
        category: input.category || 'شخصي',
        start_time: input.start_time || null,
        end_time: input.end_time || null,
        note: input.note || null,
        priority: input.priority || 'normal',
        status: 'pending',
        reschedule_count: 0,
        reminder_at: input.reminder_at || null,
        reminder_fired: false,
        updated_at: Date.now(),
        device_id: store.device_id,
        deleted_at: null
      });
      saveWebStore(store);
      return id;
    },

    async toggleTaskDone(taskId, done) {
      if (isTauri) return window.__TAURI__.core.invoke('toggle_task_done', { taskId, done });
      const store = loadWebStore();
      const t = store.tasks.find((x) => x.id === taskId);
      if (!t) return 0;
      t.status = done ? 'done' : 'pending';
      t.updated_at = Date.now();
      saveWebStore(store);
      return store.tasks.filter((x) => x.logical_date === t.logical_date && x.status === 'done' && !x.deleted_at).length;
    },

    async rescheduleTask(taskId, toDate, reason) {
      if (isTauri) return window.__TAURI__.core.invoke('reschedule_task', { taskId, toDate, reason });
      if (!reason || reason.trim().length < 3) throw new Error('يجب كتابة سبب واضح للتأجيل (3 أحرف على الأقل)');
      const store = loadWebStore();
      const t = store.tasks.find((x) => x.id === taskId);
      if (!t) throw new Error('المهمة غير موجودة');
      const fromDate = t.logical_date;
      t.logical_date = toDate;
      t.status = 'pending';
      t.reschedule_count = (t.reschedule_count || 0) + 1;
      t.updated_at = Date.now();
      store.reschedule_events.push({
        id: uuid(),
        task_id: taskId,
        from_date: fromDate,
        to_date: toDate,
        reason: reason.trim(),
        created_at: Date.now(),
        device_id: store.device_id
      });
      saveWebStore(store);
    },

    async cancelTaskWithExcuse(taskId, excuseCategory, excuseText) {
      if (isTauri) {
        return window.__TAURI__.core.invoke('cancel_task_with_excuse', { taskId, excuseCategory, excuseText });
      }
      if (!excuseText || excuseText.trim().length < 5) {
        throw new Error('لا يمكن الإلغاء بدون عذر صريح (5 أحرف على الأقل)');
      }
      const store = loadWebStore();
      const t = store.tasks.find((x) => x.id === taskId);
      if (!t) throw new Error('المهمة غير موجودة');
      t.status = 'cancelled';
      t.updated_at = Date.now();
      store.cancellation_excuses.push({
        id: uuid(),
        task_id: taskId,
        logical_date: t.logical_date,
        excuse_category: excuseCategory,
        excuse_text: excuseText.trim(),
        created_at: Date.now(),
        device_id: store.device_id
      });
      saveWebStore(store);
    },

    async togglePrayer(logicalDate, prayerKey, completed) {
      if (isTauri) return window.__TAURI__.core.invoke('toggle_prayer', { logicalDate, prayerKey, completed });
      const store = loadWebStore();
      store.prayers_log[`${logicalDate}:${prayerKey}`] = {
        logical_date: logicalDate,
        prayer_key: prayerKey,
        completed: Boolean(completed),
        updated_at: Date.now(),
        device_id: store.device_id
      };
      saveWebStore(store);
    },

    async toggleWeeklyShield(logicalDate, freeze) {
      if (isTauri) return window.__TAURI__.core.invoke('toggle_weekly_shield', { logicalDate, freeze });
      const store = loadWebStore();
      const wk = window.MasarTime.getWeekKey(logicalDate);
      if (freeze) {
        const conflict = Object.values(store.day_summaries).find(
          (s) => s.frozen && s.shield_week_key === wk && s.logical_date !== logicalDate
        );
        if (conflict) throw new Error(`درع الإجازة الأسبوعي مستخدم بالفعل في ${conflict.logical_date}`);
      }
      materializeWebDay(store, logicalDate);
      store.day_summaries[logicalDate].frozen = Boolean(freeze);
      store.day_summaries[logicalDate].shield_week_key = freeze ? wk : null;
      store.day_summaries[logicalDate].updated_at = Date.now();
      saveWebStore(store);
    },

    async logFocusSession(logicalDate, plannedMins, actualMins, completed) {
      if (isTauri) {
        return window.__TAURI__.core.invoke('log_focus_session', { logicalDate, plannedMins, actualMins, completed });
      }
      const store = loadWebStore();
      const id = uuid();
      store.focus_sessions.push({
        id,
        logical_date: logicalDate,
        started_at: Date.now(),
        ended_at: Date.now(),
        planned_mins: plannedMins,
        actual_mins: actualMins,
        completed: Boolean(completed),
        updated_at: Date.now(),
        device_id: store.device_id,
        deleted_at: null
      });
      saveWebStore(store);
      return id;
    },

    async addDistraction(logicalDate, text) {
      if (isTauri) return window.__TAURI__.core.invoke('add_distraction', { logicalDate, text });
      const store = loadWebStore();
      const id = uuid();
      store.distractions.unshift({
        id,
        logical_date: logicalDate,
        session_id: null,
        text: text.trim(),
        resolved: false,
        created_at: Date.now(),
        updated_at: Date.now(),
        device_id: store.device_id,
        deleted_at: null
      });
      saveWebStore(store);
      return id;
    },

    async listNotes() {
      if (isTauri) return window.__TAURI__.core.invoke('list_notes');
      const store = loadWebStore();
      return store.notes
        .filter((n) => !n.deleted_at)
        .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updated_at - a.updated_at);
    },

    async saveNote(input) {
      if (isTauri) return window.__TAURI__.core.invoke('save_note', { input });
      const store = loadWebStore();
      const now = Date.now();
      if (input.id) {
        const idx = store.notes.findIndex((n) => n.id === input.id);
        if (idx >= 0) {
          store.notes[idx] = { ...store.notes[idx], ...input, updated_at: now };
          saveWebStore(store);
          return input.id;
        }
      }
      const id = uuid();
      store.notes.unshift({
        id,
        logical_date: input.logical_date || null,
        title: input.title.trim(),
        body_md: input.body_md || '',
        tags_json: input.tags_json || '[]',
        pinned: Boolean(input.pinned),
        created_at: now,
        updated_at: now,
        device_id: store.device_id,
        deleted_at: null
      });
      saveWebStore(store);
      return id;
    },

    async deleteNote(noteId) {
      if (isTauri) return window.__TAURI__.core.invoke('delete_note', { noteId });
      const store = loadWebStore();
      const n = store.notes.find((x) => x.id === noteId);
      if (n) { n.deleted_at = Date.now(); n.updated_at = Date.now(); saveWebStore(store); }
    },

    async getMonthlyTelemetry(monthPrefix) {
      if (isTauri) return window.__TAURI__.core.invoke('get_monthly_telemetry', { monthPrefix });
      const store = loadWebStore();
      const dates = Object.keys(store.day_summaries).filter((d) => d.startsWith(monthPrefix)).sort();
      let sumAll = 0, sumShift = 0, cntShift = 0, sumOff = 0, cntOff = 0;
      let prayersCompleted = 0, tasksDone = 0, tasksCancelled = 0, totalFocusMins = 0;
      const dailyScores = {};
      for (const d of dates) {
        const b = await this.getDayBundle(d);
        dailyScores[d] = b.score.total_pct;
        sumAll += b.score.total_pct;
        if (b.summary.is_shift_day) { sumShift += b.score.total_pct; cntShift++; }
        else { sumOff += b.score.total_pct; cntOff++; }
        prayersCompleted += b.score.prayers_done;
        tasksDone += b.score.tasks_done;
        tasksCancelled += b.tasks.filter((t) => t.status === 'cancelled').length;
        totalFocusMins += b.score.focus_mins;
      }
      const tasksRescheduled = store.reschedule_events.filter((e) => e.from_date.startsWith(monthPrefix)).length;
      const excuseBreakdown = {};
      store.cancellation_excuses
        .filter((e) => e.logical_date.startsWith(monthPrefix))
        .forEach((e) => { excuseBreakdown[e.excuse_category] = (excuseBreakdown[e.excuse_category] || 0) + 1; });

      return {
        month_prefix: monthPrefix,
        days_logged: dates.length,
        avg_score_pct: dates.length ? Math.round((sumAll / dates.length) * 10) / 10 : 0,
        shift_days_avg_pct: cntShift ? Math.round((sumShift / cntShift) * 10) / 10 : 0,
        off_days_avg_pct: cntOff ? Math.round((sumOff / cntOff) * 10) / 10 : 0,
        prayers_completed: prayersCompleted,
        prayers_possible: dates.length * 5,
        tasks_done: tasksDone,
        tasks_rescheduled: tasksRescheduled,
        tasks_cancelled: tasksCancelled,
        total_focus_mins: totalFocusMins,
        excuse_breakdown: excuseBreakdown,
        daily_scores: dailyScores
      };
    },

    async importV5Backup(rawJson) {
      if (isTauri) return window.__TAURI__.core.invoke('import_v5_backup', { rawJson });
      const parsed = JSON.parse(rawJson);
      const v5 = parsed.state || parsed;
      const store = loadWebStore();
      const now = Date.now();
      let rules_imported = 0, tasks_imported = 0, days_imported = 0, prayers_imported = 0;

      if (v5.schedule && typeof v5.schedule === 'object') {
        store.recurrence_rules = [];
        Object.entries(v5.schedule).forEach(([dayStr, blocks]) => {
          (blocks || []).forEach((b, idx) => {
            store.recurrence_rules.push({
              id: b.id || uuid(),
              weekday: Number(dayStr),
              title: b.title || 'مهمة',
              category: b.category || 'شخصي',
              start_time: b.start || '09:00',
              end_time: b.end || null,
              note: b.note || null,
              is_work_shift: b.category === 'عمل',
              sort_order: idx,
              active: true,
              updated_at: now,
              device_id: store.device_id,
              deleted_at: null
            });
            rules_imported++;
          });
        });
      }

      const deletedSet = new Set(v5.deletedExtraIds || []);
      (v5.extraTasks || []).forEach((t) => {
        const id = t.id || uuid();
        const existingIdx = store.tasks.findIndex((x) => x.id === id);
        const mapped = {
          id,
          logical_date: t.date,
          original_logical_date: t.date,
          rule_id: null,
          title: t.title,
          category: t.category || 'شخصي',
          start_time: t.time || null,
          end_time: null,
          note: t.note || null,
          priority: t.priority || 'normal',
          status: t.status === 'done' ? 'done' : (t.status === 'skip' ? 'cancelled' : 'pending'),
          reschedule_count: 0,
          reminder_at: t.reminderAt || null,
          reminder_fired: Boolean(t.reminderFired),
          updated_at: t.updatedAt || now,
          device_id: store.device_id,
          deleted_at: deletedSet.has(id) ? now : null
        };
        if (existingIdx >= 0) store.tasks[existingIdx] = mapped;
        else store.tasks.push(mapped);
        tasks_imported++;
      });

      Object.entries(v5.history || {}).forEach(([dateKey, rec]) => {
        store.day_summaries[dateKey] = {
          logical_date: dateKey,
          is_shift_day: false,
          frozen: Boolean(rec.frozen),
          shield_week_key: rec.frozen ? window.MasarTime.getWeekKey(dateKey) : null,
          updated_at: now,
          device_id: store.device_id
        };
        days_imported++;
        Object.entries(rec.prayers || {}).forEach(([pKey, val]) => {
          if (!PRAYERS.includes(pKey)) return;
          store.prayers_log[`${dateKey}:${pKey}`] = {
            logical_date: dateKey,
            prayer_key: pKey,
            completed: Boolean(val),
            updated_at: (rec.prayersUpdatedAt && rec.prayersUpdatedAt[pKey]) || now,
            device_id: store.device_id
          };
          prayers_imported++;
        });
        if (rec.focusMinutes > 0) {
          store.focus_sessions.push({
            id: uuid(),
            logical_date: dateKey,
            started_at: now,
            ended_at: now,
            planned_mins: rec.focusMinutes,
            actual_mins: rec.focusMinutes,
            completed: true,
            updated_at: now,
            device_id: store.device_id,
            deleted_at: null
          });
        }
      });

      saveWebStore(store);
      return { rules_imported, tasks_imported, days_imported, prayers_imported };
    },

    async runGistSync(pat, gistId) {
      if (isTauri) return window.__TAURI__.core.invoke('run_gist_sync', { pat, gistId });
      const store = loadWebStore();
      const headers = {
        Authorization: `Bearer ${pat.trim()}`,
        Accept: 'application/vnd.github+json'
      };
      if (store.meta.lastSyncEtag) headers['If-None-Match'] = store.meta.lastSyncEtag;
      const res = await fetch(`https://api.github.com/gists/${gistId.trim()}`, { headers });
      if (res.status !== 304 && !res.ok) throw new Error(`GitHub Gist GET HTTP ${res.status}`);
      if (res.status === 200) {
        const data = await res.json();
        const file = data.files && data.files['abdullah_masar_v6_sync.json'];
        if (file && file.content) {
          const cloud = JSON.parse(file.content);
          // Merge tasks by LWW
          const map = new Map(store.tasks.map((t) => [t.id, t]));
          (cloud.tasks || []).forEach((ct) => {
            const lt = map.get(ct.id);
            if (!lt || ct.updated_at > lt.updated_at || (ct.updated_at === lt.updated_at && ct.device_id >= lt.device_id)) {
              map.set(ct.id, ct);
            }
          });
          store.tasks = Array.from(map.values());
        }
      }
      const patchRes = await fetch(`https://api.github.com/gists/${gistId.trim()}`, {
        method: 'PATCH',
        headers: { ...headers, 'Content-Type': 'application/json' },
        body: JSON.stringify({
          files: { 'abdullah_masar_v6_sync.json': { content: JSON.stringify(store, null, 2) } }
        })
      });
      if (!patchRes.ok) throw new Error(`GitHub Gist PATCH HTTP ${patchRes.status}`);
      const newEtag = patchRes.headers.get('ETag');
      store.meta.lastSyncEtag = newEtag;
      saveWebStore(store);
      return { status: 'synced', etag: newEtag, merged_tasks: store.tasks.length };
    },

    async startNativeTimer(durationSecs, label) {
      if (!isTauri) return;
      try {
        await window.__TAURI__.core.invoke('plugin:masar-timer|start_foreground_timer', {
          payload: { duration_secs: durationSecs, label }
        });
      } catch (_) {}
    },

    async stopNativeTimer() {
      if (!isTauri) return;
      try {
        await window.__TAURI__.core.invoke('plugin:masar-timer|stop_foreground_timer');
      } catch (_) {}
    },

    getQuotes() {
      return loadWebStore().quotes.filter((q) => !q.deleted_at);
    },

    addCustomQuote(category, text, source) {
      const store = loadWebStore();
      store.quotes.unshift({
        id: uuid(),
        category,
        text: text.trim(),
        source: (source || '').trim(),
        is_builtin: false,
        is_favorite: true,
        updated_at: Date.now(),
        device_id: store.device_id,
        deleted_at: null
      });
      saveWebStore(store);
    },

    getRules() {
      return loadWebStore().recurrence_rules.filter((r) => !r.deleted_at);
    },

    addRule(rule) {
      const store = loadWebStore();
      store.recurrence_rules.push({
        id: uuid(),
        weekday: Number(rule.weekday),
        title: rule.title.trim(),
        category: rule.category || 'شخصي',
        start_time: rule.start_time || '09:00',
        end_time: rule.end_time || null,
        note: rule.note || null,
        is_work_shift: rule.category === 'عمل',
        sort_order: store.recurrence_rules.length,
        active: true,
        updated_at: Date.now(),
        device_id: store.device_id,
        deleted_at: null
      });
      saveWebStore(store);
    },

    deleteRule(ruleId) {
      const store = loadWebStore();
      const r = store.recurrence_rules.find((x) => x.id === ruleId);
      if (r) { r.deleted_at = Date.now(); r.updated_at = Date.now(); saveWebStore(store); }
    },

    getCredentials() {
      try { return JSON.parse(localStorage.getItem(CRED_KEY) || '{}'); } catch (_) { return {}; }
    },
    saveCredentials(pat, gistId) {
      localStorage.setItem(CRED_KEY, JSON.stringify({ pat: pat.trim(), gistId: gistId.trim() }));
    }
  };

  window.MasarBridge = api;
})();
