export interface Lesson {
  subject: string;
  room: string;
  teacher: string;
  full?: boolean;
}
export interface Row {
  n: number;
  cells: Array<Lesson | null>;
}
export interface Section {
  groups: string[];
  rows: Row[];
}
export interface Day {
  date: string;
  sections: Section[];
}
export interface BellLesson {
  k: 'l';
  n: number;
  s: string;
  e: string;
}
export interface BellBreak {
  k: 'b';
  t: string;
  s: string;
  e: string;
}
export type Bell = BellLesson | BellBreak;
export interface ScheduleConfig {
  testTime: string | null;
  refreshSeconds: number;
  showBreaks: boolean;
  showPairTimes: boolean;
  showClock: boolean;
  showNoLesson: boolean;
  highlightCurrent: boolean;
  dimPast: boolean;
  dimOpacity: number;
  groupFont: number;
  groupPadY: number;
  groupPadX: number;
  pairFont: number;
  pairTimeFont: number;
  lessonFont: number;
  roomFont: number;
  teacherFont: number;
  lessonPadY: number;
  lessonPadX: number;
  breakFont: number;
  breakPadY: number;
  rowGap: number;
  colGap: number;
  radius: number;
  bells: Record<string, Bell[]>;
  bellDays: Record<string, string>;
}
export type ScheduleUserConfig = Partial<ScheduleConfig> & {
  genHtml?: boolean;
};
