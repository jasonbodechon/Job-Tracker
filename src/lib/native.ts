import type { CompanyDirectoryEntry, Diagnostics, Job, JobSourceConfig, JobStatus, SearchProfile, SearchRun, StartedSearch, TrackerDetails } from "../types/job";

const API = import.meta.env.VITE_API_URL ?? "";
export class ApiError extends Error { constructor(message: string, public status: number) { super(message); } }
async function request<T>(path: string, init: RequestInit = {}): Promise<T> { const response = await fetch(`${API}${path}`, { credentials: "include", ...init, headers: { ...(init.body instanceof FormData ? {} : { "Content-Type": "application/json" }), ...init.headers } }); if (!response.ok) { const data = await response.json().catch(() => ({})); throw new ApiError(data.error ?? `Request failed (${response.status})`, response.status); } if (response.status === 204) return undefined as T; return response.json() as Promise<T>; }
const body = (value: unknown): RequestInit => ({ method: "POST", body: JSON.stringify(value) });

export interface SessionUser { id: string; email: string; displayName: string; role: "user" | "admin"; }
export interface AdminOverview { users:number; admins:number; profiles:number; enabledSources:number; activeJobs:number; companies:number; database:string; }
export interface AdminCompanyInput { id:string; companyName:string; countries:string[]; atsProvider:string; boardIdentifier:string; careersUrl:string; identifierOrigin:"Manual"|"Automatic"; }
export const loadSession = () => request<SessionUser>("/api/auth/session");
export const loadSiteSchedule = () => request<{schedule:SearchProfile["schedule"]}>("/api/site-schedule");
export const login = (email: string, password: string) => request<SessionUser>("/api/auth/login", body({ email, password }));
export const register = (email: string, password: string, displayName: string) => request<SessionUser>("/api/auth/register", body({ email, password, displayName }));
export const logout = () => request<void>("/api/auth/logout", { method: "POST" });
export const loadStoredJobs = () => request<Job[]>("/api/jobs");
export const persistTrackerStatus = (jobId: string, status: JobStatus) => request<void>(`/api/jobs/${encodeURIComponent(jobId)}/status`, { method: "PUT", body: JSON.stringify({ status }) });
export const loadProfiles = () => request<SearchProfile[]>("/api/profiles");
export const persistProfile = (profile: SearchProfile) => request<void>("/api/profiles", body(profile));
export const removeProfile = (profileId: string) => request<void>(`/api/profiles/${encodeURIComponent(profileId)}`, { method: "DELETE" });
export const loadSources = () => request<JobSourceConfig[]>("/api/sources");
export const loadCompanyDirectory = () => request<CompanyDirectoryEntry[]>("/api/company-directory");
export const verifyCompanyDirectoryEntry = (id: string) => request<CompanyDirectoryEntry>(`/api/company-directory/${encodeURIComponent(id)}/verify`, { method: "POST" });
export const setCompanyDirectoryEnabled = (id: string, enabled: boolean) => request<CompanyDirectoryEntry>(`/api/company-directory/${encodeURIComponent(id)}/enabled`, { method: "PUT", body: JSON.stringify({ enabled }) });
export const addCompanyDirectorySource = (id: string) => request<JobSourceConfig>(`/api/company-directory/${encodeURIComponent(id)}/add-source`, { method: "POST" });
export const removeCompanyDirectorySources = (id: string) => request<number>(`/api/company-directory/${encodeURIComponent(id)}/source`, { method: "DELETE" });
export const loadAdminOverview = () => request<AdminOverview>("/api/admin/overview");
export const loadAdminDiagnostics = () => request<Diagnostics>("/api/admin/diagnostics");
export const runAdminRetention = (retentionDays:number) => request<number>("/api/admin/retention", { method:"POST",body:JSON.stringify({retentionDays}) });
export const loadAdminUsers = () => request<SessionUser[]>("/api/admin/users");
export const setAdminUserRole = (id:string, role:"user"|"admin") => request<SessionUser>(`/api/admin/users/${encodeURIComponent(id)}/role`, { method:"PUT",body:JSON.stringify({role}) });
export const setAllSchedules = (schedule:SearchProfile["schedule"]) => request<number>("/api/admin/schedules",{method:"PUT",body:JSON.stringify({schedule})});
export const saveAdminCompany = (company:AdminCompanyInput) => request<void>("/api/admin/companies",body(company));
export const deleteAdminCompany = (id:string) => request<number>(`/api/admin/companies/${encodeURIComponent(id)}`,{method:"DELETE"});
export const persistSource = (source: JobSourceConfig, usaJobsEmail = "", usaJobsApiKey = "") => request<void>("/api/sources", body({ ...source, usaJobsEmail: usaJobsEmail || undefined, usaJobsApiKey: usaJobsApiKey || undefined }));
export const removeSource = (sourceId: string) => request<void>(`/api/sources/${encodeURIComponent(sourceId)}`, { method: "DELETE" });
export const loadSearchRuns = () => request<SearchRun[]>("/api/search-runs");
export const refreshSavedSource = (sourceId: string) => request<StartedSearch>(`/api/sources/${encodeURIComponent(sourceId)}/refresh`, { method: "POST" });
export const runServerSearch = (triggerType = "manual") => request<StartedSearch>("/api/searches/run", body({ triggerType }));
export const loadSearchRun = (runId: string) => request<SearchRun>(`/api/search-runs/${encodeURIComponent(runId)}`);
export const loadTracker = (jobId: string) => request<TrackerDetails>(`/api/jobs/${encodeURIComponent(jobId)}/tracker`);
export const persistTracker = (tracker: TrackerDetails) => request<void>(`/api/jobs/${encodeURIComponent(tracker.jobId)}/tracker`, { method: "PUT", body: JSON.stringify(tracker) });
export const loadDiagnostics = () => request<Diagnostics>("/api/diagnostics");
export const runRetention = (retentionDays: number) => request<number>("/api/retention", body({ retentionDays }));
function download(path: string, filename: string) { const link = document.createElement("a"); link.href = `${API}${path}`; link.download = filename; document.body.appendChild(link); link.click(); link.remove(); }
export function exportArchive() { download("/api/export/archive", `job-tracker-archive-${new Date().toISOString().slice(0, 10)}.json`); }
export function exportJobsCsv() { download("/api/export/jobs.csv", `job-tracker-${new Date().toISOString().slice(0, 10)}.csv`); }
export async function importDesktopDatabase(file: File): Promise<{ profiles: number; sources: number; jobs: number }> { const data = new FormData(); data.append("file", file); return request("/api/import/desktop", { method: "POST", body: data }); }
export async function importWebArchive(file: File): Promise<{ profiles: number; sources: number; jobs: number; trackers?: number }> { return request("/api/import/archive", body(JSON.parse(await file.text()))); }
export interface AppNotification { id: string; title: string; body: string; url: string; read: boolean; createdAt: string; }
export const loadNotifications = () => request<AppNotification[]>("/api/notifications");
export const markNotificationsRead = () => request<void>("/api/notifications/read", { method: "POST" });
export async function enablePushNotifications(): Promise<boolean> { if (!("serviceWorker" in navigator) || !("PushManager" in window)) return false; const permission = await Notification.requestPermission(); if (permission !== "granted") return false; const { vapidPublicKey } = await request<{ vapidPublicKey?: string }>("/api/push/config"); if (!vapidPublicKey) throw new Error("Web Push is not configured on this server."); const registration = await navigator.serviceWorker.ready; const subscription = await registration.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: decodeKey(vapidPublicKey) }); await request<void>("/api/push/subscriptions", body(subscription.toJSON())); return true; }
function decodeKey(value: string): ArrayBuffer { const padded = value.padEnd(Math.ceil(value.length / 4) * 4, "=").replace(/-/g, "+").replace(/_/g, "/"); return Uint8Array.from(atob(padded), (char) => char.charCodeAt(0)).buffer as ArrayBuffer; }
