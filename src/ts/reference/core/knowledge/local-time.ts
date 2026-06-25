export function localTimeZoneName(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "local";
}

export function formatLocalTimestamp(isoTimestamp: string | null | undefined): string | null {
  if (!isoTimestamp) return null;
  const date = new Date(isoTimestamp);
  if (Number.isNaN(date.getTime())) return isoTimestamp;
  return [
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`,
    `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`,
    localOffset(date),
  ].join(" ");
}

function localOffset(date: Date): string {
  const offsetMinutes = -date.getTimezoneOffset();
  const sign = offsetMinutes >= 0 ? "+" : "-";
  const absolute = Math.abs(offsetMinutes);
  const hours = Math.floor(absolute / 60);
  const minutes = absolute % 60;
  return `UTC${sign}${pad(hours)}:${pad(minutes)}`;
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}
