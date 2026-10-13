import SchedulePage from './SchedulePage';
import type { Day, ScheduleUserConfig } from './scheduleTypes';
const DAY: Day = __DATA__;
const USERCFG: ScheduleUserConfig = __USERCFG__;
export default function GeneratedSchedule() {
  return <SchedulePage day={DAY} userConfig={USERCFG} />;
}
