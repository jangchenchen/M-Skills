export type SmartAddKind = "url" | "local" | "askAi" | "empty";

export function classifySmartAddInput(raw: string): SmartAddKind {
  const s = raw.trim();
  if (!s) return "empty";
  if (/^https?:\/\//i.test(s)) return "url";
  if (/^git@[\w.-]+:[\w./~-]+/.test(s)) return "url";
  if (/^ssh:\/\//i.test(s)) return "url";
  if (/^file:\/\//i.test(s)) return "local";
  if (
    s.startsWith("/") ||
    s.startsWith("~/") ||
    s.startsWith("./") ||
    s.startsWith("../")
  ) {
    return "local";
  }
  if (/^[A-Za-z]:[\\/]/.test(s)) return "local";
  if (/^\\\\/.test(s)) return "local";
  if (/\s/.test(s)) return "askAi";
  if (s.includes("/")) return "local";
  return "askAi";
}
