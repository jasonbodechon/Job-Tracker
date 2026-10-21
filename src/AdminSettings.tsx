import { useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { findDirectorySource } from "./lib/companyDirectory";
import { verificationDetails, verificationSeverity } from "./lib/verificationStatus";
import {
  deleteAdminCompany,
  exportArchive,
  exportJobsCsv,
  loadAdminDiagnostics,
  loadAdminOverview,
  loadAdminUsers,
  loadSiteSchedule,
  runAdminRetention,
  saveAdminCompany,
  setAdminUserRole,
  setAllSchedules,
  type AdminCompanyInput,
  type AdminOverview,
  type SessionUser,
} from "./lib/native";
import type { CompanyDirectoryEntry, Diagnostics, JobSourceConfig, JobSourceKind, SearchProfile } from "./types/job";

type Tab = "Overview" | "Companies" | "Sources" | "Schedules" | "Users" | "Data";

const providers: JobSourceKind[] = ["Greenhouse", "Lever", "Ashby", "SmartRecruiters", "Amazon", "Workday", "Microsoft", "Google", "CareerPage", "Adzuna"];
const blankCompany: AdminCompanyInput = { id: "", companyName: "", countries: ["Canada", "United States"], atsProvider: "SmartRecruiters", boardIdentifier: "", careersUrl: "", identifierOrigin: "Manual" };

interface Props {
  directory: CompanyDirectoryEntry[];
  sources: JobSourceConfig[];
  sourceManagement: ReactNode;
  onReload: () => Promise<void>;
  onToggleCompany: (entry: CompanyDirectoryEntry) => Promise<void>;
  onAddSource: (entry: CompanyDirectoryEntry) => Promise<void>;
  onRemoveSource: (entry: CompanyDirectoryEntry) => Promise<void>;
  onVerify: (entry: CompanyDirectoryEntry) => Promise<void>;
  onImport: (file: File | undefined) => Promise<void>;
  onMessage: (message: string) => void;
  onSiteScheduleChanged: (schedule: SearchProfile["schedule"]) => void;
}

export default function AdminSettings({ directory, sources, sourceManagement, onReload, onToggleCompany, onAddSource, onRemoveSource, onVerify, onImport, onMessage, onSiteScheduleChanged }: Props) {
  const [tab, setTab] = useState<Tab>("Overview");
  const [overview, setOverview] = useState<AdminOverview | null>(null);
  const [siteDiagnostics, setSiteDiagnostics] = useState<Diagnostics | null>(null);
  const [retentionDays, setRetentionDays] = useState(365);
  const [users, setUsers] = useState<SessionUser[]>([]);
  const [company, setCompany] = useState<AdminCompanyInput>(blankCompany);
  const [globalSchedule, setGlobalSchedule] = useState<SearchProfile["schedule"]>("Manual");
  const [companyQuery, setCompanyQuery] = useState("");
  const [countryFilter, setCountryFilter] = useState("");
  const [providerFilter, setProviderFilter] = useState("");

  const visibleCompanies = directory.filter((entry) =>
    (!companyQuery || `${entry.companyName} ${entry.boardIdentifier}`.toLowerCase().includes(companyQuery.toLowerCase()))
    && (!countryFilter || entry.countries.includes(countryFilter))
    && (!providerFilter || entry.atsProvider === providerFilter));

  async function refreshAdmin() {
    const [nextOverview, nextDiagnostics, nextUsers, nextSchedule] = await Promise.all([
      loadAdminOverview(), loadAdminDiagnostics(), loadAdminUsers(), loadSiteSchedule(),
    ]);
    setOverview(nextOverview);
    setSiteDiagnostics(nextDiagnostics);
    setUsers(nextUsers);
    setGlobalSchedule(nextSchedule.schedule);
  }

  useEffect(() => { void refreshAdmin(); }, []);

  async function saveCompany() {
    await saveAdminCompany(company);
    setCompany(blankCompany);
    await onReload();
    await refreshAdmin();
    onMessage("Company directory entry saved for all users.");
  }

  async function removeCompany(entry: CompanyDirectoryEntry) {
    if (!confirm(`Delete ${entry.companyName} from the site-wide Company directory? Saved user jobs are retained.`)) return;
    await deleteAdminCompany(entry.id);
    await onReload();
    await refreshAdmin();
    onMessage(`${entry.companyName} removed from the Company directory.`);
  }

  function editCompany(entry: CompanyDirectoryEntry) {
    setCompany({ id: entry.id, companyName: entry.companyName, countries: entry.countries, atsProvider: entry.atsProvider, boardIdentifier: entry.boardIdentifier, careersUrl: entry.careersUrl, identifierOrigin: entry.identifierOrigin });
  }

  return <section className="admin-shell">
    <div className="admin-tabs">{(["Overview", "Companies", "Sources", "Schedules", "Users", "Data"] as Tab[]).map((value) => <button className={tab === value ? "active" : ""} onClick={() => setTab(value)} key={value}>{value}</button>)}</div>

    {tab === "Overview" && <section className="settings-card admin-card"><h2>Site administration</h2><p>Changes here apply across Job-Tracker. Personal profile metrics and job recommendations are intentionally hidden from administration pages.</p>{overview && <div className="diagnostics-grid"><span>Users<strong>{overview.users}</strong></span><span>Administrators<strong>{overview.admins}</strong></span><span>Profiles<strong>{overview.profiles}</strong></span><span>Enabled sources<strong>{overview.enabledSources}</strong></span><span>Active jobs<strong>{overview.activeJobs}</strong></span><span>Companies<strong>{overview.companies}</strong></span></div>}</section>}

    {tab === "Companies" && <>
      <section className="settings-card admin-card"><h2>Add or edit company</h2><div className="admin-form-grid"><label>ID<input value={company.id} onChange={(e) => setCompany({ ...company, id: e.target.value })} placeholder="company-slug"/></label><label>Company name<input value={company.companyName} onChange={(e) => setCompany({ ...company, companyName: e.target.value })}/></label><label>Connector<select value={company.atsProvider} onChange={(e) => setCompany({ ...company, atsProvider: e.target.value })}>{providers.map((value) => <option key={value}>{value}</option>)}</select></label><label>Board identifier / search keyword<input value={company.boardIdentifier} onChange={(e) => setCompany({ ...company, boardIdentifier: e.target.value })}/></label><label>Careers URL{(company.atsProvider === "Workday" || company.atsProvider === "CareerPage") && " (required)"}<input value={company.careersUrl} onChange={(e) => setCompany({ ...company, careersUrl: e.target.value })} placeholder="https://…"/></label><label>Countries<input value={company.countries.join(", ")} onChange={(e) => setCompany({ ...company, countries: e.target.value.split(",").map((value) => value.trim()).filter(Boolean) })}/></label></div><div className="inline-actions"><button className="primary" onClick={() => void saveCompany()}>Save company</button><button onClick={() => setCompany(blankCompany)}>Clear</button></div><p>SmartRecruiters uses its company identifier, Workday uses the public tenant careers URL, and Adzuna uses the board identifier as its search keyword. Adzuna credentials remain configured in the server environment.</p></section>
      <section className="settings-card directory-card admin-card"><h2>Company directory</h2><div className="directory-controls"><input value={companyQuery} onChange={(e) => setCompanyQuery(e.target.value)} placeholder="Search company or identifier"/><select value={countryFilter} onChange={(e) => setCountryFilter(e.target.value)}><option value="">All countries</option><option>Canada</option><option>United States</option></select><select value={providerFilter} onChange={(e) => setProviderFilter(e.target.value)}><option value="">All ATS providers</option>{providers.map((value) => <option key={value}>{value}</option>)}</select></div><div className="table-wrap directory-table"><table><thead><tr><th>COMPANY</th><th>COUNTRIES</th><th>PROVIDER</th><th>IDENTIFIER</th><th>JOBS</th><th>LAST VERIFIED</th><th>STATUS</th><th>ACTIONS</th></tr></thead><tbody>{visibleCompanies.map((entry) => { const saved = findDirectorySource(entry, sources); return <tr key={entry.id}><td><label className="directory-company-toggle"><input type="checkbox" checked={entry.enabled} onChange={() => void onToggleCompany(entry)}/><span><strong>{entry.companyName}</strong><small>{entry.connectorClass} · {entry.identifierOrigin}</small></span></label></td><td>{entry.countries.join(", ") || "Not classified"}</td><td>{entry.atsProvider}</td><td><code>{entry.boardIdentifier}</code></td><td>{entry.currentJobCount}</td><td>{entry.lastVerifiedAt ? new Date(entry.lastVerifiedAt).toLocaleString() : "Not yet"}</td><td><VerificationStatus entry={entry}/></td><td><div className="directory-actions"><button onClick={() => editCompany(entry)}>Edit</button><button disabled={!entry.enabled || Boolean(saved)} onClick={() => void onAddSource(entry)}>Add source</button><button disabled={!saved} onClick={() => void onRemoveSource(entry)}>Remove source</button><button onClick={() => void onVerify(entry)}>Verify</button><button className="remove-source" onClick={() => void removeCompany(entry)}>Delete</button></div></td></tr>; })}</tbody></table>{visibleCompanies.length === 0 && <p className="empty-directory">No companies match these filters.</p>}</div></section>
      {visibleCompanies.some((entry) => entry.verificationError) && <section className="settings-card connector-diagnostics"><h3>Company connector diagnostics</h3>{visibleCompanies.filter((entry) => entry.verificationError).map((entry) => <p key={entry.id} className="source-error"><b>{entry.companyName} · {entry.verificationStatus}</b><br/>{entry.verificationError}</p>)}</section>}
    </>}

    {tab === "Sources" && sourceManagement}

    {tab === "Schedules" && <section className="settings-card admin-card"><h2>Site-wide schedule</h2><p>Applying this schedule updates every saved profile. Users can see the current site-wide value at the top of their Schedules page but cannot edit it there.</p><div className="inline-actions"><select value={globalSchedule} onChange={(e) => setGlobalSchedule(e.target.value as SearchProfile["schedule"])}><option>Manual</option><option>Daily</option><option>Weekdays</option><option>Weekly</option></select><button className="primary" onClick={async () => { const count = await setAllSchedules(globalSchedule); onSiteScheduleChanged(globalSchedule); await refreshAdmin(); onMessage(`${count} profile schedules updated.`); }}>Apply to all profiles</button></div></section>}

    {tab === "Users" && <section className="settings-card admin-card"><h2>User roles</h2><div className="saved-sources">{users.map((value) => <article key={value.id}><div><strong>{value.displayName}</strong><small>{value.email}</small></div><select value={value.role} onChange={async (e) => { const updated = await setAdminUserRole(value.id, e.target.value as "user" | "admin"); setUsers((current) => current.map((item) => item.id === updated.id ? updated : item)); }}><option value="user">User</option><option value="admin">Administrator</option></select></article>)}</div></section>}

    {tab === "Data" && <section className="settings-card admin-card"><h2>Data management and diagnostics</h2><p>Diagnostics and retention are site-wide. Archive/CSV export and import apply to the administrator account; use the PostgreSQL backup script for a complete site backup.</p><div className="inline-actions"><button onClick={exportArchive}>Export my archive</button><button onClick={exportJobsCsv}>Export my CSV</button><label className="file-button">Import into my account<input type="file" accept=".sqlite3,.db,.json" onChange={(e) => void onImport(e.target.files?.[0])}/></label><label>Retention days<input type="number" min="7" max="3650" value={retentionDays} onChange={(e) => setRetentionDays(Number(e.target.value))}/></label><button onClick={async () => { const removed = await runAdminRetention(retentionDays); await refreshAdmin(); onMessage(`Site-wide retention removed ${removed} old records.`); }}>Run site-wide retention</button></div>{siteDiagnostics && <div className="diagnostics-grid"><span>Active jobs<strong>{siteDiagnostics.activeJobs}</strong></span><span>Inactive jobs<strong>{siteDiagnostics.inactiveJobs}</strong></span><span>Profiles<strong>{siteDiagnostics.profiles}</strong></span><span>Enabled sources<strong>{siteDiagnostics.enabledSources}</strong></span><span>Search runs<strong>{siteDiagnostics.searchRuns}</strong></span><span>Source errors<strong>{siteDiagnostics.sourceErrors}</strong></span><span>Follow-ups due<strong>{siteDiagnostics.dueFollowUps}</strong></span><span>Database size<strong>{siteDiagnostics.databaseSizeBytes.toLocaleString()} bytes</strong></span></div>}</section>}
  </section>;
}

function VerificationStatus({ entry }: { entry: CompanyDirectoryEntry }) {
  const badge = useRef<HTMLSpanElement>(null);
  const [position, setPosition] = useState<{ top: number; left: number } | null>(null);
  const details = verificationDetails(entry);
  const show = () => {
    if (!details.length || !badge.current) return;
    const bounds = badge.current.getBoundingClientRect();
    setPosition({ top: Math.max(12, Math.min(bounds.bottom + 8, window.innerHeight - 220)), left: Math.max(12, Math.min(bounds.left, window.innerWidth - 392)) });
  };
  const hide = () => setPosition(null);
  const tooltipId = `verification-details-${entry.id.replace(/[^a-zA-Z0-9_-]/g, "-")}`;
  return <>
    <span ref={badge} className={`verification-status status-${verificationSeverity(entry.verificationStatus)}${details.length ? " has-details" : ""}`} tabIndex={details.length ? 0 : undefined} aria-describedby={details.length ? tooltipId : undefined} onMouseEnter={show} onMouseLeave={hide} onFocus={show} onBlur={hide}>{entry.verificationStatus}</span>
    {position && createPortal(<div id={tooltipId} role="tooltip" className="verification-tooltip" style={position}><strong>{entry.companyName} verification</strong><dl><div><dt>Status</dt><dd>{entry.verificationStatus}</dd></div>{details.map((detail) => <div key={`${detail.label}-${detail.value}`}><dt>{detail.label}</dt><dd>{detail.value}</dd></div>)}</dl></div>, document.body)}
  </>;
}
