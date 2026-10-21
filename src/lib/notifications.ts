export async function notify(title: string, body: string): Promise<boolean> {
  if (typeof window === "undefined" || !("Notification" in window)) return false;
  const permission = Notification.permission === "default" ? await Notification.requestPermission() : Notification.permission;
  if (permission !== "granted") return false;
  new Notification(title, { body, icon: "/icon.svg" });
  return true;
}
