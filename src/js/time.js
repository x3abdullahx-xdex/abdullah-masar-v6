// Abdullah Masar V6 — 4:00 AM Logical Day Cutoff & Arabic Date Helpers
(function () {
  const CUTOFF_HOURS = 4;
  const AR_DAYS = ['الأحد', 'الاثنين', 'الثلاثاء', 'الأربعاء', 'الخميس', 'الجمعة', 'السبت'];
  const AR_MONTHS = [
    'يناير', 'فبراير', 'مارس', 'أبريل', 'مايو', 'يونيو',
    'يوليو', 'أغسطس', 'سبتمبر', 'أكتوبر', 'نوفمبر', 'ديسمبر'
  ];

  function pad2(n) { return String(n).padStart(2, '0'); }

  function getLogicalNow() {
    return new Date(Date.now() - CUTOFF_HOURS * 3600000);
  }

  function isExtendedNight() {
    return new Date().getHours() < CUTOFF_HOURS;
  }

  function toDateKey(d) {
    return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
  }

  function getLogicalTodayKey() {
    return toDateKey(getLogicalNow());
  }

  function parseDateKey(dateKey) {
    const [y, m, d] = dateKey.split('-').map(Number);
    return new Date(y, m - 1, d, 12, 0, 0);
  }

  function getWeekday(dateKey) {
    return parseDateKey(dateKey).getDay();
  }

  function formatArabicDate(dateKey) {
    const d = parseDateKey(dateKey);
    return `${AR_DAYS[d.getDay()]}، ${d.getDate()} ${AR_MONTHS[d.getMonth()]} ${d.getFullYear()}`;
  }

  function getArabicDayName(dateKey) {
    return AR_DAYS[getWeekday(dateKey)];
  }

  function addDays(dateKey, delta) {
    const d = parseDateKey(dateKey);
    d.setDate(d.getDate() + delta);
    return toDateKey(d);
  }

  function getMonthPrefix(dateKey) {
    return dateKey.slice(0, 7);
  }

  function getWeekKey(dateKey) {
    const d = parseDateKey(dateKey);
    const tmp = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()));
    const dayNum = tmp.getUTCDay() || 7;
    tmp.setUTCDate(tmp.getUTCDate() + 4 - dayNum);
    const yearStart = new Date(Date.UTC(tmp.getUTCFullYear(), 0, 1));
    const weekNo = Math.ceil((((tmp - yearStart) / 86400000) + 1) / 7);
    return `${tmp.getUTCFullYear()}-W${pad2(weekNo)}`;
  }

  window.MasarTime = {
    CUTOFF_HOURS,
    AR_DAYS,
    AR_MONTHS,
    getLogicalNow,
    isExtendedNight,
    toDateKey,
    getLogicalTodayKey,
    parseDateKey,
    getWeekday,
    formatArabicDate,
    getArabicDayName,
    addDays,
    getMonthPrefix,
    getWeekKey
  };
})();
