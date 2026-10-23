import { existsSync, readFileSync, readdirSync } from "node:fs";
import { describe, expect, it } from "vitest";

const root = process.cwd();
const local = "Start Job Tracker Local.cmd";
const production = "Deploy Job Tracker Production.cmd";
const read = (name: string) => readFileSync(`${root}/${name}`, "utf8").toLowerCase();

describe("CMD-only Docker launchers", () => {
  it("ships only the current CMD lifecycle scripts and no PowerShell files", () => {
    expect(readdirSync(root).filter((name) => name.toLowerCase().endsWith(".ps1"))).toEqual([]);
    expect(existsSync(`${root}/${local}`)).toBe(true);
    expect(existsSync(`${root}/${production}`)).toBe(true);
    for (const retired of ["Job Tracker Installer.cmd", "Job Tracker Uninstaller.cmd", "Verify Job Tracker.cmd"]) expect(existsSync(`${root}/${retired}`)).toBe(false);
    for (const name of [local, production]) expect(read(name)).not.toContain("powershell");
  });

  it("uses Windows CRLF line endings so CMD subroutine labels remain discoverable", () => {
    for (const name of [local, production]) {
      const contents = readFileSync(`${root}/${name}`, "utf8");
      expect(contents).toContain("\r\n");
      expect(contents.replace(/\r\n/g, "")).not.toContain("\n");
    }
  });

  it("installs prerequisites, checks high/critical audits, and validates the frontend", () => {
    for (const name of [local, production]) {
      const script = read(name);
      for (const command of ["winget install --id docker.dockerdesktop", "winget install --id openjs.nodejs.lts", "npm ci", "npm audit --audit-level=high", "npm run test:frontend", "docker compose"]) expect(script).toContain(command);
    }
    const productionScript = read(production);
    for (const command of ["npm audit fix --force", "npm run build"]) expect(productionScript).toContain(command);
  });

  it("only reports audit findings locally and never rewrites dependencies at startup", () => {
    const script = read(local);
    expect(script).not.toContain("npm audit fix");
    expect(script).toContain("warning: npm audit reported high or critical package issues");
  });

  it("leaves the frontend build to the Docker image instead of building it twice", () => {
    expect(read(local)).not.toContain("npm run build");
    expect(readFileSync(`${root}/Dockerfile`, "utf8")).toContain("RUN npm run build");
  });

  it("backs up an existing local database before upgrading and keeps backups out of git and Docker builds", () => {
    const script = read(local);
    expect(script).toContain("call :backup_database");
    expect(script).toContain("pg_dump -u job_tracker -d job_tracker -fc");
    expect(script).toContain("backups\\job-tracker-");
    expect(script.indexOf("call :backup_database")).toBeLessThan(script.indexOf("up -d --remove-orphans"));
    expect(script).toContain("nothing was upgraded");
    expect(readFileSync(`${root}/.gitignore`, "utf8")).toMatch(/^backups\/$/m);
    expect(readFileSync(`${root}/.dockerignore`, "utf8")).toMatch(/^backups\/$/m);
  });

  it("generates local secrets and checks Node.js with the cryptographic helper", () => {
    const script = read(local);
    expect(script).not.toContain("!random!-");
    expect(script).toContain("node deployment\\local-tools.mjs secret");
    expect(script).toContain("node deployment\\local-tools.mjs check-node %jt_min_node_major%");
    expect(script).toContain('set "jt_min_node_major=20"');
  });

  it("keeps CMD labels outside parenthesized blocks so goto loops stay reliable", () => {
    let depth = 0;
    const misplaced: string[] = [];
    for (const line of readFileSync(`${root}/${local}`, "utf8").split(/\r\n/)) {
      const trimmed = line.trim();
      if (trimmed.startsWith(":") && !trimmed.startsWith("::") && depth > 0) misplaced.push(trimmed);
      const code = trimmed.replace(/"[^"]*"/g, "").replace(/\^./g, "");
      if (/^rem\b/i.test(code)) continue;
      for (const character of code) { if (character === "(") depth += 1; if (character === ")") depth = Math.max(0, depth - 1); }
    }
    expect(misplaced).toEqual([]);
    expect(read(local)).toContain("call :wait_for_docker");
    expect(read(local)).toContain("restart windows");
  });

  it("offers a quick start that skips tests and rebuilds for an existing installation", () => {
    const script = read(local);
    expect(script).toContain('if /i "%~1"=="quick" set "jt_quick=1"');
    expect(script).toContain("if defined jt_quick goto :start_services");
    expect(script).toContain("quick start needs an existing installation");
  });

  it("passes the local database password to psql as a quoted variable", () => {
    const script = read(local);
    expect(script).toContain("with password :'jt_password';");
    expect(script).toContain('-v "jt_password=!jt_postgres_password!"');
    expect(script).not.toContain("with password '!jt_postgres_password!'");
  });

  it("uses the right URL and Compose configuration for each mode", () => {
    expect(read(local)).toContain("docker-compose.local.yml");
    expect(read(local)).toContain("http://localhost:8080");
    const script = read(production);
    expect(script).toContain("cookie_secure must be true");
    expect(script).toContain("https://!public_url!");
    expect(script).toContain("job_tracker_domain must be a public dns name");
  });

  it("upgrades running Compose services without deleting persistent volumes", () => {
    for (const name of [local, production]) {
      const script = read(name);
      expect(script).toContain("pull postgres caddy");
      expect(script).toContain("build --pull");
      expect(script).toContain("up -d --remove-orphans");
      expect(script).not.toContain("down -v");
      expect(script).not.toContain("docker volume rm");
      expect(script).not.toContain("install_and_verify_frontend");
    }
  });

  it("selects a new installation or update while keeping a stable Compose identity", () => {
    for (const name of [local, production]) {
      const script = read(name);
      expect(script).toContain("call :resolve_install_mode");
      expect(script).toContain("deployment_mode=new install");
      expect(script).toContain("deployment_mode=update");
      expect(script).toContain("compose_project_name=job-tracker");
      expect(script).toContain("_job_tracker_postgres");
      expect(script).toContain("_caddy_data");
    }
  });

  it("releases port 8080 only from an older Job-Tracker Caddy container", () => {
    const script = read(local);
    expect(script).toContain("call :release_local_port");
    expect(script).toContain('publish=8080');
    expect(script).toContain('findstr /i /c:"caddy"');
    expect(script).toContain('findstr /i /c:"job-tracker" /c:"jobtracker"');
    expect(script).toContain("which is not a job-tracker proxy");
  });
});
