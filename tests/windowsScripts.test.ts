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

  it("installs prerequisites, repairs high/critical audits, and validates the frontend", () => {
    for (const name of [local, production]) {
      const script = read(name);
      for (const command of ["winget install --id docker.dockerdesktop", "winget install --id openjs.nodejs.lts", "npm ci", "npm audit --audit-level=high", "npm audit fix --force", "npm run test:frontend", "npm run build", "docker compose"]) expect(script).toContain(command);
    }
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
