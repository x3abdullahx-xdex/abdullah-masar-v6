// Abdullah Masar V6 — Main UI Controller (100% Offline, RTL, Tauri v2 + Browser Fallback)
(function () {
  const T = window.MasarTime;
  const A = window.MasarAudio;
  const B = window.MasarBridge;

  const PRAYER_META = [
    { key: 'fajr', label: 'الفجر' },
    { key: 'dhuhr', label: 'الظهر' },
    { key: 'asr', label: 'العصر' },
    { key: 'maghrib', label: 'المغرب' },
    { key: 'isha', label: 'العشاء' }
  ];

  const EXCUSE_LABELS = {
    shift_overload: 'ضغط شيفت / شغل',
    health: 'إرهاق أو ظرف صحي',
    emergency: 'ظرف طارئ',
    poor_planning: 'سوء تقدير للوقت',
    other: 'سبب آخر'
  };

  let currentDate = T.getLogicalTodayKey();
  let currentBundle = null;
  let currentQuoteIdx = 0;

  // Zen Focus state (Date.now() authoritative + 2-min checkpoints)
  const ZEN_CKPT_KEY = 'masar_v6_zen_checkpoint';
  let zenPlannedMins = 25;
  let zenRemainingSecs = 25 * 60;
  let zenEndEpoch = null;
  let zenInterval = null;
  let zenLastCkpt = 0;

  const $ = (id) => document.getElementById(id);

  function showToast(msg) {
    const el = $('toast');
    el.textContent = msg;
    el.classList.remove('hidden');
    clearTimeout(el._t);
    el._t = setTimeout(() => el.classList.add('hidden'), 2800);
  }

  function escapeHtml(s) {
    return String(s || '')
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
  }

  function updateHeaderClock() {
    const isNight = T.isExtendedNight();
    const badge = $('nightBadge');
    if (isNight) {
      badge.textContent = `🌙 امتداد يوم ${T.getArabicDayName(T.getLogicalTodayKey())}`;
      badge.classList.remove('hidden');
    } else {
      badge.classList.add('hidden');
    }
    $('dateSubtitle').textContent = `${T.formatArabicDate(currentDate)} • طب بيطري عين شمس × Airbnb`;
  }

  function renderQuote() {
    const quotes = B.getQuotes();
    if (!quotes.length) return;
    const q = quotes[currentQuoteIdx % quotes.length];
    $('dailyQuoteText').textContent = q.text;
    $('dailyQuoteSource').textContent = `${q.source ? q.source + ' • ' : ''}${q.category}`;
  }

  async function renderDaily() {
    updateHeaderClock();
    currentBundle = await B.getDayBundle(currentDate);
    const b = currentBundle;

    // 1. 5 Daily Prayers
    const pGrid = $('prayersGrid');
    pGrid.innerHTML = PRAYER_META.map((p) => {
      const done = Boolean(b.prayers[p.key]);
      return `<button class="prayer-btn ${done ? 'done' : ''}" data-prayer="${p.key}">
        <div>${done ? '✓' : '○'}</div>
        <div>${p.label}</div>
      </button>`;
    }).join('');
    $('prayersCount').textContent = `${b.score.prayers_done} / 5`;

    // 2. Fair Shift-Day Score
    $('scoreHeading').textContent = b.score.is_shift_day ? 'ميزان يوم الشيفت (عادل)' : 'ميزان يوم الإجازة';
    $('scoreFormulaLabel').textContent = b.score.is_shift_day
      ? `الصلوات 45% + المهام 55% + بونص تركيز (+10% كحد أقصى) • تركيز اليوم: ${b.score.focus_mins} د`
      : `الصلوات 45% + المهام 40% + التركيز 15% • تركيز اليوم: ${b.score.focus_mins} د`;
    $('scorePct').textContent = `${b.score.total_pct}%`;
    $('scoreFill').style.width = `${Math.min(100, b.score.total_pct)}%`;

    // 3. Shield Button
    $('btnShield').textContent = b.summary.frozen ? '🛡️ الدرع مفعّل' : '🛡️ درع الإجازة';

    // 4. Tasks List (No Free Skip — only Done, Reschedule with reason, Cancel with excuse)
    const list = $('taskList');
    if (!b.tasks.length) {
      list.innerHTML = '<div class="muted">لا توجد مهام مسجلة لهذا اليوم.</div>';
    } else {
      list.innerHTML = b.tasks.map((t) => {
        const isDone = t.status === 'done';
        const isCancelled = t.status === 'cancelled';
        return `<div class="task-row ${isDone ? 'done' : ''} ${isCancelled ? 'cancelled' : ''}">
          <div class="task-main">
            <button class="task-check ${isDone ? 'checked' : ''}" data-toggle-task="${t.id}" data-next="${!isDone}">
              ${isDone ? '✓' : ''}
            </button>
            <div>
              <div class="task-title">${escapeHtml(t.title)}</div>
              <div class="task-meta">
                <span class="chip">${escapeHtml(t.category)}</span>
                ${t.start_time ? `<span>⏰ ${escapeHtml(t.start_time)}${t.end_time ? ' – ' + escapeHtml(t.end_time) : ''}</span>` : ''}
                ${t.reschedule_count > 0 ? `<span class="chip chip-warn">أُجّلت ${t.reschedule_count}×</span>` : ''}
                ${isCancelled ? `<span class="chip chip-danger">ملغاة بعذر</span>` : ''}
                ${t.note ? `<span>${escapeHtml(t.note)}</span>` : ''}
              </div>
            </div>
          </div>
          ${!isDone && !isCancelled ? `
            <div class="task-actions">
              <button class="btn btn-sm" data-resched="${t.id}" title="تأجيل بسبب">تأجيل</button>
              <button class="btn btn-sm btn-danger" data-cancel="${t.id}" title="إلغاء بعذر إجباري">إلغاء</button>
            </div>` : ''}
        </div>`;
      }).join('');
    }

    await renderMiniCalendar();
    renderRadar();
    renderDistractions();
  }

  async function renderMiniCalendar() {
    const prefix = T.getMonthPrefix(currentDate);
    const tel = await B.getMonthlyTelemetry(prefix);
    const [y, m] = prefix.split('-').map(Number);
    const daysInMonth = new Date(y, m, 0).getDate();
    const grid = $('miniCalGrid');
    let html = '';
    for (let d = 1; d <= daysInMonth; d++) {
      const dk = `${prefix}-${String(d).padStart(2, '0')}`;
      const sc = tel.daily_scores[dk];
      let cls = '';
      if (sc !== undefined) {
        cls = sc >= 80 ? 'lvl-high' : (sc >= 60 ? 'lvl-mid' : 'lvl-low');
      }
      html += `<button class="cal-cell ${cls} ${dk === currentDate ? 'selected' : ''}" data-cal-date="${dk}">
        <strong>${d}</strong>
        <span style="font-size:0.62rem;">${sc !== undefined ? Math.round(sc) + '%' : ''}</span>
      </button>`;
    }
    grid.innerHTML = html;
  }

  function renderRadar() {
    const el = $('radarList');
    const upcoming = (currentBundle ? currentBundle.tasks : [])
      .filter((t) => !t.rule_id && t.status === 'pending')
      .slice(0, 5);
    if (!upcoming.length) {
      el.innerHTML = '<div class="muted">لا توجد مواعيد خاصة قادمة في رادار اليوم.</div>';
      return;
    }
    el.innerHTML = upcoming.map((t) => `
      <div class="task-row">
        <div>
          <div class="task-title">${escapeHtml(t.title)}</div>
          <div class="task-meta"><span>${t.logical_date}</span>${t.start_time ? `<span>⏰ ${t.start_time}</span>` : ''}</div>
        </div>
      </div>`).join('');
  }

  async function renderMonthly() {
    const picker = $('monthPicker');
    if (!picker.value) picker.value = T.getMonthPrefix(currentDate);
    const tel = await B.getMonthlyTelemetry(picker.value);
    const prayerPct = tel.prayers_possible ? Math.round((tel.prayers_completed / tel.prayers_possible) * 100) : 0;

    $('monthlyKpis').innerHTML = `
      <div class="kpi-box"><div class="kpi-label">الأيام المسجلة</div><div class="kpi-val">${tel.days_logged}</div></div>
      <div class="kpi-box"><div class="kpi-label">متوسط الالتزام</div><div class="kpi-val">${tel.avg_score_pct}%</div></div>
      <div class="kpi-box"><div class="kpi-label">متوسط أيام الشيفت</div><div class="kpi-val">${tel.shift_days_avg_pct}%</div></div>
      <div class="kpi-box"><div class="kpi-label">متوسط أيام الإجازة</div><div class="kpi-val">${tel.off_days_avg_pct}%</div></div>
      <div class="kpi-box"><div class="kpi-label">الصلوات (${tel.prayers_completed}/${tel.prayers_possible})</div><div class="kpi-val">${prayerPct}%</div></div>
      <div class="kpi-box"><div class="kpi-label">المهام المنجزة</div><div class="kpi-val">${tel.tasks_done}</div></div>
      <div class="kpi-box"><div class="kpi-label">مرات التأجيل</div><div class="kpi-val">${tel.tasks_rescheduled}</div></div>
      <div class="kpi-box"><div class="kpi-label">المهام الملغاة بعذر</div><div class="kpi-val">${tel.tasks_cancelled}</div></div>
      <div class="kpi-box"><div class="kpi-label">ساعات التركيز</div><div class="kpi-val">${(tel.total_focus_mins / 60).toFixed(1)} س</div></div>
    `;

    const ex = Object.entries(tel.excuse_breakdown || {});
    $('excuseStats').innerHTML = ex.length
      ? ex.map(([cat, cnt]) => `<div class="task-row"><span>${EXCUSE_LABELS[cat] || cat}</span><strong>${cnt}</strong></div>`).join('')
      : '<div class="muted">لا توجد إلغاءات مسجلة في هذا الشهر.</div>';
  }

  // Zen Focus Studio
  function formatMMSS(secs) {
    const s = Math.max(0, Math.floor(secs));
    const m = Math.floor(s / 60);
    const r = s % 60;
    return `${String(m).padStart(2, '0')}:${String(r).padStart(2, '0')}`;
  }

  function updateZenUI() {
    $('zenClock').textContent = formatMMSS(zenRemainingSecs);
  }

  function startZen() {
    if (zenInterval) return;
    zenEndEpoch = Date.now() + zenRemainingSecs * 1000;
    $('zenModeBadge').textContent = 'قيد التركيز';
    B.startNativeTimer(zenRemainingSecs, 'جلسة تركيز Zen — المسار اليومي');
    zenInterval = setInterval(async () => {
      zenRemainingSecs = Math.max(0, Math.round((zenEndEpoch - Date.now()) / 1000));
      updateZenUI();
      if (Date.now() - zenLastCkpt > 120000) {
        zenLastCkpt = Date.now();
        localStorage.setItem(ZEN_CKPT_KEY, JSON.stringify({
          logicalDate: currentDate,
          plannedMins: zenPlannedMins,
          remainingSecs: zenRemainingSecs,
          savedAt: Date.now()
        }));
      }
      if (zenRemainingSecs <= 0) {
        await stopZen(true);
      }
    }, 500);
  }

  function pauseZen() {
    if (!zenInterval) return;
    clearInterval(zenInterval);
    zenInterval = null;
    $('zenModeBadge').textContent = 'متوقف مؤقتاً';
    B.stopNativeTimer();
  }

  async function stopZen(completed) {
    pauseZen();
    localStorage.removeItem(ZEN_CKPT_KEY);
    const elapsedMins = Math.max(1, Math.round((zenPlannedMins * 60 - zenRemainingSecs) / 60));
    if (elapsedMins >= 1) {
      await B.logFocusSession(currentDate, zenPlannedMins, completed ? zenPlannedMins : elapsedMins, completed);
      A.playCompletionRing(12);
      showToast(`تم تسجيل ${completed ? zenPlannedMins : elapsedMins} دقيقة تركيز ✓`);
    }
    zenRemainingSecs = zenPlannedMins * 60;
    $('zenModeBadge').textContent = 'جاهز';
    updateZenUI();
    await renderDaily();
  }

  function checkZenRecovery() {
    try {
      const raw = localStorage.getItem(ZEN_CKPT_KEY);
      if (!raw) return;
      const ck = JSON.parse(raw);
      const banner = $('zenRecovery');
      banner.textContent = `تم العثور على جلسة غير مكتملة (${ck.plannedMins} د) — متبقي ${Math.ceil(ck.remainingSecs / 60)} د.`;
      banner.classList.remove('hidden');
      zenPlannedMins = ck.plannedMins;
      zenRemainingSecs = ck.remainingSecs;
      updateZenUI();
    } catch (_) {}
  }

  function renderDistractions() {
    const list = $('distractionsList');
    const items = currentBundle ? currentBundle.distractions : [];
    list.innerHTML = items.length
      ? items.map((d) => `<div class="task-row"><span>${escapeHtml(d.text)}</span></div>`).join('')
      : '<div class="muted">لا توجد مشتتات مسجلة اليوم.</div>';
  }

  // Notes
  async function renderNotes() {
    const notes = await B.listNotes();
    const list = $('notesList');
    list.innerHTML = notes.length
      ? notes.map((n) => `
          <div class="task-row">
            <div style="flex:1; cursor:pointer;" data-edit-note="${n.id}">
              <div class="task-title">${n.pinned ? '📌 ' : ''}${escapeHtml(n.title)}</div>
              <div class="muted">${escapeHtml((n.body_md || '').slice(0, 90))}</div>
            </div>
            <button class="btn btn-sm btn-danger" data-del-note="${n.id}">حذف</button>
          </div>`).join('')
      : '<div class="muted">لا توجد ملاحظات بعد.</div>';
  }

  // Weekly Schedule Editor
  function renderScheduleEditor() {
    const sel = $('schedDaySelect');
    if (!sel.options.length) {
      T.AR_DAYS.forEach((name, idx) => {
        const opt = document.createElement('option');
        opt.value = idx;
        opt.textContent = name;
        sel.appendChild(opt);
      });
      sel.value = T.getWeekday(currentDate);
    }
    const wd = Number(sel.value);
    const rules = B.getRules().filter((r) => r.weekday === wd);
    $('rulesList').innerHTML = rules.length
      ? rules.map((r) => `
          <div class="task-row">
            <div>
              <div class="task-title">${escapeHtml(r.title)}</div>
              <div class="task-meta"><span class="chip">${escapeHtml(r.category)}</span><span>⏰ ${r.start_time}${r.end_time ? ' – ' + r.end_time : ''}</span></div>
            </div>
            <button class="btn btn-sm btn-danger" data-del-rule="${r.id}">حذف</button>
          </div>`).join('')
      : '<div class="muted">لا توجد قواعد مجدولة لهذا اليوم.</div>';
  }

  // Quotes Library
  function renderQuotesLibrary() {
    const cat = $('quoteCatFilter').value;
    const all = B.getQuotes();
    const filtered = cat === 'all' ? all : all.filter((q) => q.category === cat);
    $('quotesList').innerHTML = filtered.map((q) => `
      <div class="task-row">
        <div>
          <div style="font-family:var(--font-serif); font-size:1.05rem;">${escapeHtml(q.text)}</div>
          <div class="task-meta"><span class="chip">${escapeHtml(q.category)}</span>${q.source ? `<span>${escapeHtml(q.source)}</span>` : ''}</div>
        </div>
      </div>`).join('');
  }

  // Dual WhatsApp Export
  function copyWhatsApp(mode) {
    if (!currentBundle) return;
    const b = currentBundle;
    const header = mode === 'plan'
      ? `📋 خطة يوم ${T.formatArabicDate(currentDate)}`
      : `📊 تقرير إنجاز ${T.formatArabicDate(currentDate)} (${b.score.total_pct}%)`;
    const prayersLine = `🕌 الصلوات: ${b.score.prayers_done}/5`;
    const focusLine = `⏱️ التركيز: ${b.score.focus_mins} دقيقة`;
    const tasksLines = b.tasks.map((t) => {
      const mark = t.status === 'done' ? '✅' : (t.status === 'cancelled' ? '❌' : '⬜');
      return `${mark} ${t.start_time ? `[${t.start_time}] ` : ''}${t.title}`;
    }).join('\n');
    const text = `${header}\n${prayersLine} | ${focusLine}\n\n${tasksLines}`;
    navigator.clipboard.writeText(text).then(() => showToast('تم النسخ للحافظة (جاهز للواتساب) ✓')).catch(() => showToast('تعذر النسخ التلقائي'));
  }

  // .ics Export
  function exportIcs() {
    if (!currentBundle) return;
    const dt = currentDate.replace(/-/g, '');
    const lines = ['BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//Abdullah Masar V6//AR'];
    currentBundle.tasks.filter((t) => t.start_time && t.status !== 'cancelled').forEach((t) => {
      const hhmm = t.start_time.replace(':', '') + '00';
      lines.push('BEGIN:VEVENT', `UID:${t.id}@masar.v6`, `DTSTART:${dt}T${hhmm}`, `SUMMARY:${t.title}`, 'END:VEVENT');
    });
    lines.push('END:VCALENDAR');
    const blob = new Blob([lines.join('\r\n')], { type: 'text/calendar;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `masar-${currentDate}.ics`;
    a.click();
    URL.revokeObjectURL(url);
    showToast('تم تصدير ملف .ics ✓');
  }

  function bindEvents() {
    // Tabs
    $('navTabs').addEventListener('click', async (e) => {
      const btn = e.target.closest('[data-tab]');
      if (!btn) return;
      const tab = btn.dataset.tab;
      document.querySelectorAll('.nav-tab').forEach((b) => b.classList.toggle('active', b === btn));
      document.querySelectorAll('.tab-panel').forEach((p) => p.classList.toggle('hidden', p.id !== `tab-${tab}`));
      if (tab === 'monthly') await renderMonthly();
      if (tab === 'notes') await renderNotes();
      if (tab === 'schedule') renderScheduleEditor();
      if (tab === 'quotes') renderQuotesLibrary();
    });

    // Header actions
    $('btnTheme').onclick = () => {
      const html = document.documentElement;
      html.dataset.theme = html.dataset.theme === 'dark' ? 'light' : 'dark';
    };
    $('btnCalm').onclick = () => {
      const html = document.documentElement;
      const next = html.dataset.calm !== 'true';
      html.dataset.calm = String(next);
      showToast(next ? 'تم تفعيل الوضع الهادئ 🌙' : 'تم إيقاف الوضع الهادئ');
    };
    $('btnMute').onclick = () => {
      const next = !A.isMuted();
      A.setMuted(next);
      $('btnMute').textContent = next ? '🔇' : '🔊';
    };
    $('btnAddTask').onclick = () => $('modalAddTask').classList.remove('hidden');
    $('btnNextQuote').onclick = () => { currentQuoteIdx++; renderQuote(); };

    // Date navigation
    $('btnPrevDay').onclick = () => { currentDate = T.addDays(currentDate, -1); renderDaily(); };
    $('btnToday').onclick = () => { currentDate = T.getLogicalTodayKey(); renderDaily(); };
    $('btnNextDay').onclick = () => { currentDate = T.addDays(currentDate, 1); renderDaily(); };

    // Prayers
    $('prayersGrid').addEventListener('click', async (e) => {
      const btn = e.target.closest('[data-prayer]');
      if (!btn || !currentBundle) return;
      const key = btn.dataset.prayer;
      const next = !currentBundle.prayers[key];
      await B.togglePrayer(currentDate, key, next);
      if (next) A.playPrayerTone();
      await renderDaily();
    });

    // Weekly Shield
    $('btnShield').onclick = async () => {
      if (!currentBundle) return;
      try {
        await B.toggleWeeklyShield(currentDate, !currentBundle.summary.frozen);
        await renderDaily();
        showToast('تم تحديث درع الإجازة الأسبوعي 🛡️');
      } catch (err) {
        showToast(err.message || String(err));
      }
    };

    // Task Actions (Done / Reschedule / Cancel)
    $('taskList').addEventListener('click', async (e) => {
      const chk = e.target.closest('[data-toggle-task]');
      if (chk) {
        const done = chk.dataset.next === 'true';
        const doneCount = await B.toggleTaskDone(chk.dataset.toggleTask, done);
        if (done) A.playCompletionRing(doneCount);
        await renderDaily();
        return;
      }
      const rBtn = e.target.closest('[data-resched]');
      if (rBtn) {
        $('reschedTaskId').value = rBtn.dataset.resched;
        $('reschedDate').value = T.addDays(currentDate, 1);
        $('reschedReason').value = '';
        $('modalReschedule').classList.remove('hidden');
        return;
      }
      const cBtn = e.target.closest('[data-cancel]');
      if (cBtn) {
        $('cancelTaskId').value = cBtn.dataset.cancel;
        $('cancelText').value = '';
        $('modalCancel').classList.remove('hidden');
      }
    });

    // Mini Calendar Day Click
    $('miniCalGrid').addEventListener('click', (e) => {
      const cell = e.target.closest('[data-cal-date]');
      if (!cell) return;
      currentDate = cell.dataset.calDate;
      renderDaily();
    });

    // Modals close & submit
    document.querySelectorAll('[data-close]').forEach((b) => {
      b.onclick = () => $(b.dataset.close).classList.add('hidden');
    });

    $('btnSubmitTask').onclick = async () => {
      const title = $('newTaskTitle').value.trim();
      if (!title) return showToast('اكتب عنوان المهمة أولاً');
      await B.createTask({
        logical_date: currentDate,
        title,
        category: $('newTaskCat').value,
        start_time: $('newTaskTime').value || null,
        note: $('newTaskNote').value.trim() || null
      });
      $('newTaskTitle').value = '';
      $('newTaskNote').value = '';
      $('modalAddTask').classList.add('hidden');
      await renderDaily();
      showToast('تمت إضافة المهمة ✓');
    };

    $('btnSubmitResched').onclick = async () => {
      try {
        await B.rescheduleTask($('reschedTaskId').value, $('reschedDate').value, $('reschedReason').value);
        $('modalReschedule').classList.add('hidden');
        await renderDaily();
        showToast('تم تأجيل المهمة وتسجيل السبب ✓');
      } catch (err) {
        showToast(err.message || String(err));
      }
    };

    $('btnSubmitCancel').onclick = async () => {
      try {
        await B.cancelTaskWithExcuse($('cancelTaskId').value, $('cancelCat').value, $('cancelText').value);
        $('modalCancel').classList.add('hidden');
        await renderDaily();
        showToast('تم إلغاء المهمة وتسجيل العذر');
      } catch (err) {
        showToast(err.message || String(err));
      }
    };

    // WhatsApp & ICS
    $('btnWaPlan').onclick = () => copyWhatsApp('plan');
    $('btnWaReport').onclick = () => copyWhatsApp('report');
    $('btnExportIcs').onclick = exportIcs;

    // Monthly picker
    $('monthPicker').onchange = renderMonthly;

    // Zen Focus
    document.querySelectorAll('.zen-preset').forEach((b) => {
      b.onclick = () => {
        pauseZen();
        zenPlannedMins = Number(b.dataset.mins);
        zenRemainingSecs = zenPlannedMins * 60;
        updateZenUI();
      };
    });
    $('btnZenStart').onclick = startZen;
    $('btnZenPause').onclick = pauseZen;
    $('btnZenStop').onclick = () => stopZen(false);
    $('ambientSelect').onchange = () => A.setAmbient($('ambientSelect').value, Number($('ambientVol').value));
    $('ambientVol').oninput = () => A.setAmbientVolume(Number($('ambientVol').value));

    $('btnAddDistraction').onclick = async () => {
      const txt = $('distractionInput').value.trim();
      if (!txt) return;
      await B.addDistraction(currentDate, txt);
      $('distractionInput').value = '';
      await renderDaily();
    };

    // Notes
    $('btnClearNote').onclick = () => {
      $('noteEditId').value = '';
      $('noteTitle').value = '';
      $('noteBody').value = '';
      $('notePinned').checked = false;
    };
    $('btnSaveNote').onclick = async () => {
      const title = $('noteTitle').value.trim();
      if (!title) return showToast('اكتب عنواناً للملاحظة');
      await B.saveNote({
        id: $('noteEditId').value || null,
        logical_date: currentDate,
        title,
        body_md: $('noteBody').value,
        pinned: $('notePinned').checked
      });
      $('btnClearNote').click();
      await renderNotes();
      showToast('تم حفظ الملاحظة ✓');
    };
    $('notesList').addEventListener('click', async (e) => {
      const del = e.target.closest('[data-del-note]');
      if (del) { await B.deleteNote(del.dataset.delNote); await renderNotes(); return; }
      const ed = e.target.closest('[data-edit-note]');
      if (ed) {
        const notes = await B.listNotes();
        const n = notes.find((x) => x.id === ed.dataset.editNote);
        if (n) {
          $('noteEditId').value = n.id;
          $('noteTitle').value = n.title;
          $('noteBody').value = n.body_md;
          $('notePinned').checked = n.pinned;
        }
      }
    });

    // Weekly Schedule Editor
    $('schedDaySelect').onchange = renderScheduleEditor;
    $('btnAddRule').onclick = () => {
      const title = $('ruleTitle').value.trim();
      if (!title) return showToast('اكتب عنوان البلوك');
      B.addRule({
        weekday: Number($('schedDaySelect').value),
        title,
        start_time: $('ruleStart').value,
        end_time: $('ruleEnd').value,
        category: $('ruleCat').value
      });
      $('ruleTitle').value = '';
      renderScheduleEditor();
      showToast('تمت الإضافة للجدول الأسبوعي ✓');
    };
    $('rulesList').addEventListener('click', (e) => {
      const del = e.target.closest('[data-del-rule]');
      if (!del) return;
      B.deleteRule(del.dataset.delRule);
      renderScheduleEditor();
    });

    // Quotes
    $('quoteCatFilter').onchange = renderQuotesLibrary;
    $('btnAddQuote').onclick = () => {
      const text = $('newQuoteText').value.trim();
      if (!text) return;
      B.addCustomQuote($('newQuoteCat').value, text, $('newQuoteSource').value);
      $('newQuoteText').value = '';
      $('newQuoteSource').value = '';
      renderQuotesLibrary();
      showToast('تمت إضافة المقولة للمكتبة ✓');
    };

    // Sync & V5 Importer
    const creds = B.getCredentials();
    $('syncPat').value = creds.pat || '';
    $('syncGistId').value = creds.gistId || '';

    $('btnSaveSyncCreds').onclick = () => {
      B.saveCredentials($('syncPat').value, $('syncGistId').value);
      showToast('تم حفظ بيانات المزامنة محلياً ✓');
    };

    const runSync = async () => {
      const pat = $('syncPat').value.trim();
      const gistId = $('syncGistId').value.trim();
      if (!pat || !gistId) return showToast('أدخل التوكن و Gist ID أولاً');
      $('syncDot').className = 'sync-dot syncing';
      $('syncPillText').textContent = 'جاري المزامنة...';
      try {
        const res = await B.runGistSync(pat, gistId);
        $('syncDot').className = 'sync-dot synced';
        $('syncPillText').textContent = 'متزامن ✓';
        showToast(`تمت المزامنة الذكية (${res.merged_tasks} مهمة) ☁️✓`);
        await renderDaily();
      } catch (err) {
        $('syncDot').className = 'sync-dot error';
        $('syncPillText').textContent = 'خطأ مزامنة';
        showToast(err.message || String(err));
      }
    };
    $('btnRunSyncNow').onclick = runSync;
    $('syncPill').onclick = runSync;

    $('v5FileInput').onchange = async (e) => {
      const f = e.target.files && e.target.files[0];
      if (!f) return;
      const text = await f.text();
      try {
        const rep = await B.importV5Backup(text);
        $('v5ImportResult').textContent = `تم الترحيل بنجاح: ${rep.rules_imported} قاعدة جدول • ${rep.tasks_imported} مهمة • ${rep.days_imported} يوم • ${rep.prayers_imported} صلاة.`;
        showToast('تم ترحيل بيانات V5 بنجاح ✓');
        await renderDaily();
      } catch (err) {
        showToast(err.message || String(err));
      }
    };

    $('btnImportV5Local').onclick = async () => {
      const raw = localStorage.getItem('masar_os_v3_abdullah');
      if (!raw) return showToast('لم يتم العثور على بيانات V5 في هذا المتصفح — ارفع ملف JSON بدلاً من ذلك');
      try {
        const rep = await B.importV5Backup(raw);
        $('v5ImportResult').textContent = `تم الترحيل التلقائي: ${rep.rules_imported} قاعدة • ${rep.tasks_imported} مهمة • ${rep.days_imported} يوم.`;
        showToast('تم ترحيل V5 المحلي بنجاح ✓');
        await renderDaily();
      } catch (err) {
        showToast(err.message || String(err));
      }
    };
  }

  // Boot
  B.initWebSingleInstance(() => showToast('تنبيه: لوحة المسار مفتوحة بالفعل في تبويب آخر'));
  bindEvents();
  checkZenRecovery();
  renderQuote();
  renderDaily();
})();
