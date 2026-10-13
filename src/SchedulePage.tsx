import { useEffect, useMemo, useRef, useState } from 'react';
import type { CSSProperties } from 'react';
import './SchedulePage.css';
import type {
  Bell,
  BellLesson,
  Day,
  Row,
  ScheduleConfig,
  ScheduleUserConfig,
} from './scheduleTypes';
const DEF: ScheduleConfig = {
  testTime: null,
  refreshSeconds: 20,
  showBreaks: true,
  showPairTimes: true,
  showClock: true,
  showNoLesson: true,
  highlightCurrent: true,
  dimPast: true,
  dimOpacity: 0.4,
  groupFont: 1.0,
  groupPadY: 0.7,
  groupPadX: 0.6,
  pairFont: 1.05,
  pairTimeFont: 0.48,
  lessonFont: 1.0,
  roomFont: 0.78,
  teacherFont: 0.7,
  lessonPadY: 0.4,
  lessonPadX: 0.65,
  breakFont: 0.68,
  breakPadY: 0.12,
  rowGap: 0.4,
  colGap: 0.7,
  radius: 3,
  bells: {},
  bellDays: {},
};
const NUM_KEYS = [
  'groupFont',
  'groupPadY',
  'groupPadX',
  'pairFont',
  'pairTimeFont',
  'lessonFont',
  'roomFont',
  'teacherFont',
  'lessonPadY',
  'lessonPadX',
  'breakFont',
  'breakPadY',
  'rowGap',
  'colGap',
  'radius',
] as const;
const BOOL_KEYS = [
  'showBreaks',
  'showPairTimes',
  'showClock',
  'showNoLesson',
  'highlightCurrent',
  'dimPast',
] as const;
function num(v: unknown, d: number): number {
  return typeof v === 'number' && isFinite(v) && v >= 0 ? v : d;
}
function bool(v: unknown, d: boolean): boolean {
  return typeof v === 'boolean' ? v : d;
}
function normalizeConfig(user: ScheduleUserConfig = {}): ScheduleConfig {
  const cfg: ScheduleConfig = { ...DEF, ...user };
  cfg.testTime =
    typeof cfg.testTime === 'string' && /^\d{1,2}:\d{2}$/.test(cfg.testTime)
      ? cfg.testTime
      : null;
  cfg.refreshSeconds = num(cfg.refreshSeconds, 20) || 20;
  for (const k of BOOL_KEYS) cfg[k] = bool(cfg[k], true);
  cfg.dimOpacity =
    typeof cfg.dimOpacity === 'number' &&
    cfg.dimOpacity >= 0 &&
    cfg.dimOpacity <= 1
      ? cfg.dimOpacity
      : 0.4;
  for (const k of NUM_KEYS) cfg[k] = num(cfg[k], DEF[k]);
  if (!cfg.bells || typeof cfg.bells !== 'object') cfg.bells = {};
  if (!cfg.bellDays || typeof cfg.bellDays !== 'object') cfg.bellDays = {};
  return cfg;
}
function toMin(t: string): number {
  const p = t.split(':');
  return +p[0] * 60 + +p[1];
}
function nowMin(cfg: ScheduleConfig): number {
  if (cfg.testTime) {
    const p = cfg.testTime.split(':');
    if (p.length === 2 && !isNaN(+p[0]) && !isNaN(+p[1])) {
      return +p[0] * 60 + +p[1];
    }
  }
  const n = new Date();
  return n.getHours() * 60 + n.getMinutes();
}
type Slot = { t: 'l'; r: Row } | { t: 'b'; b: Extract<Bell, { k: 'b' }> };
function isLesson(b: Bell): b is BellLesson {
  return b.k === 'l';
}
function isBreak(b: Bell): b is Extract<Bell, { k: 'b' }> {
  return b.k === 'b';
}
function buildSlots(
  rows: Row[],
  bells: Bell[] | null,
  showBreaks: boolean,
): { slots: Slot[]; sorted: Row[] } {
  const sorted = [...rows].sort((a, b) => a.n - b.n);
  const slots: Slot[] = [];
  sorted.forEach((r, i) => {
    slots.push({ t: 'l', r });
    const nx = sorted[i + 1];
    if (showBreaks && bells && nx && nx.n === r.n + 1) {
      const li = bells.findIndex((x) => isLesson(x) && x.n === r.n);
      if (li >= 0) {
        const mid1 = bells[li + 1];
        const mid2 = bells[li + 2];
        if (mid1 && isBreak(mid1) && mid2 && isLesson(mid2) && mid2.n === nx.n) {
          slots.push({ t: 'b', b: mid1 });
        }
      }
    }
  });
  return { slots, sorted };
}
export interface SchedulePageProps {
  day: Day;
  userConfig?: ScheduleUserConfig;
}
export default function SchedulePage({ day, userConfig }: SchedulePageProps) {
  const cfg = useMemo(() => normalizeConfig(userConfig), [userConfig]);
  const appRef = useRef<HTMLDivElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const [clockText, setClockText] = useState('');
  const [curMin, setCurMin] = useState<number | null>(null);
  const date = useMemo(() => new Date(day.date + 'T00:00:00'), [day.date]);
  const weekday = date.toLocaleDateString('ru-RU', { weekday: 'long' });
  const dateLong = date.toLocaleDateString('ru-RU', {
    day: 'numeric',
    month: 'long',
  });
  const dayKey = cfg.bellDays[String(date.getDay())];
  const bells: Bell[] | null = dayKey ? cfg.bells[dayKey] ?? null : null;
  useEffect(() => {
    const root = document.documentElement.style;
    root.setProperty('--dim', String(cfg.dimOpacity));
    root.setProperty('--radius', cfg.radius + 'px');
    root.setProperty('--row-gap', cfg.rowGap + 'em');
    root.setProperty('--col-gap', cfg.colGap + 'em');
    root.setProperty('--gf', cfg.groupFont + 'em');
    root.setProperty('--gy', cfg.groupPadY + 'em');
    root.setProperty('--gx', cfg.groupPadX + 'em');
    root.setProperty('--pf', cfg.pairFont + 'em');
    root.setProperty('--ptf', cfg.pairTimeFont + 'em');
    root.setProperty('--lf', cfg.lessonFont + 'em');
    root.setProperty('--rf', cfg.roomFont + 'em');
    root.setProperty('--tf', cfg.teacherFont + 'em');
    root.setProperty('--ly', cfg.lessonPadY + 'em');
    root.setProperty('--lx', cfg.lessonPadX + 'em');
    root.setProperty('--bf', cfg.breakFont + 'em');
    root.setProperty('--by', cfg.breakPadY + 'em');
  }, [cfg]);
  useEffect(() => {
    document.title = 'Расписание на ' + date.toLocaleDateString('ru-RU');
  }, [date]);
  useEffect(() => {
    const tick = () => {
      if (cfg.testTime) {
        setClockText(cfg.testTime);
      } else {
        setClockText(
          new Date().toLocaleTimeString('ru-RU', {
            hour: '2-digit',
            minute: '2-digit',
          }),
        );
      }
      setCurMin(nowMin(cfg));
    };
    tick();
    const id = setInterval(tick, (cfg.refreshSeconds || 20) * 1000);
    return () => clearInterval(id);
  }, [cfg]);
  useEffect(() => {
    const overflows = (): boolean => {
      const wrap = wrapRef.current;
      if (!wrap) return false;
      if (wrap.scrollHeight > wrap.clientHeight + 1) return true;
      const cells = wrap.querySelectorAll('.c,.b');
      for (const c of cells) {
        const el = c as HTMLElement;
        if (
          el.scrollHeight > el.clientHeight + 1 ||
          el.scrollWidth > el.clientWidth + 1
        ) {
          return true;
        }
      }
      return false;
    };
    const fit = () => {
      const app = appRef.current;
      if (!app) return;
      let lo = 8;
      let hi = 56;
      while (hi - lo > 0.5) {
        const mid = (lo + hi) / 2;
        app.style.fontSize = mid + 'px';
        if (overflows()) hi = mid;
        else lo = mid;
      }
      app.style.fontSize = lo + 'px';
    };
    const onResize = () => fit();
    window.addEventListener('resize', onResize);
    const fonts = (document as Document & { fonts?: { ready: Promise<unknown> } }).fonts;
    if (fonts && fonts.ready) fonts.ready.then(() => fit());
    else fit();
    fit();
    return () => window.removeEventListener('resize', onResize);
  }, [day, cfg]);
  const mark = (s: string, e: string, isEmpty: boolean): string => {
    if (!bells || curMin === null) return '';
    if (!cfg.highlightCurrent && !cfg.dimPast) return '';
    const sm = toMin(s);
    const em = toMin(e);
    if (cfg.dimPast && em <= curMin) return ' past';
    if (cfg.highlightCurrent && sm <= curMin && !isEmpty) return ' now';
    return '';
  };
  const lessonBell = (n: number): BellLesson | null => {
    if (!cfg.showPairTimes || !bells) return null;
    return bells.find((x): x is BellLesson => isLesson(x) && x.n === n) ?? null;
  };
  return (
    <div id="app" ref={appRef}>
      <header>
        <h1>
          Расписание
          <span>
            {weekday}, {dateLong}
          </span>
        </h1>
        {cfg.showClock && <span id="clock">{clockText}</span>}
      </header>
      <div id="wrap" ref={wrapRef}>
        {day.sections.map((s, si) => {
          const { slots, sorted } = buildSlots(s.rows, bells, cfg.showBreaks);
          const last = s.groups.map((_, ci) =>
            sorted.reduce((a, r, ri) => (r.cells[ci] ? ri : a), -1),
          );
          const secStyle = {
            '--rows': sorted.length + 1,
            gridTemplateColumns:
              '3.4em repeat(' + s.groups.length + ', minmax(0,1fr))',
            gridTemplateRows:
              'auto ' +
              slots.map((x) => (x.t === 'l' ? 'minmax(0,1fr)' : 'auto')).join(' '),
          } as CSSProperties;
          return (
            <section key={si} className="sec" style={secStyle}>
              <div className="h blank" />
              {s.groups.map((g, gi) => (
                <div key={gi} className="h">
                  {g}
                </div>
              ))}
              {slots.map((x, xi) => {
                if (x.t === 'b') {
                  const b = x.b;
                  return (
                    <div
                      key={xi}
                      className={'b' + mark(b.s, b.e, false)}
                      data-s={b.s}
                      data-e={b.e}
                    >
                      <b>{b.t}</b>
                      <span>
                        {b.s}–{b.e}
                      </span>
                    </div>
                  );
                }
                const r = x.r;
                const ri = sorted.indexOf(r);
                const lb = lessonBell(r.n);
                const bellForRow =
                  bells?.find((bl): bl is BellLesson => isLesson(bl) && bl.n === r.n) ??
                  null;
                const numMark = bellForRow ? mark(bellForRow.s, bellForRow.e, false) : '';
                return [
                  <div key={'n' + xi} className={'n' + numMark} data-n={r.n}>
                    {r.n}
                    {lb && lb.k === 'l' ? (
                      <small>
                        {lb.s}–{lb.e}
                      </small>
                    ) : null}
                  </div>,
                  ...r.cells.map((l, ci) => {
                    if (l) {
                      const m =
                        bellForRow && bellForRow.k === 'l'
                          ? mark(bellForRow.s, bellForRow.e, false)
                          : '';
                      if (l.full) {
                        return (
                          <div
                            key={'c' + xi + '_' + ci}
                            className={'c grp' + (ci % 2) + ' full' + m}
                            data-n={r.n}
                          >
                            <div className="s">{l.room}</div>
                            {l.teacher ? (
                              <div className="t">{l.teacher}</div>
                            ) : null}
                          </div>
                        );
                      }
                      return (
                        <div
                          key={'c' + xi + '_' + ci}
                          className={'c grp' + (ci % 2) + m}
                          data-n={r.n}
                        >
                          <div className="s">{l.subject}</div>
                          {l.room ? <div className="r">{l.room}</div> : null}
                          {l.teacher ? (
                            <div className="t">{l.teacher}</div>
                          ) : null}
                        </div>
                      );
                    }
                    const m =
                      bellForRow && bellForRow.k === 'l'
                        ? mark(bellForRow.s, bellForRow.e, true)
                        : '';
                    return (
                      <div
                        key={'c' + xi + '_' + ci}
                        className={'c grp' + (ci % 2) + ' e' + m}
                        data-n={r.n}
                      >
                        <div className="s">
                          {cfg.showNoLesson && ri < last[ci] ? 'Нет пары' : ''}
                        </div>
                      </div>
                    );
                  }),
                ];
              })}
            </section>
          );
        })}
      </div>
    </div>
  );
}
