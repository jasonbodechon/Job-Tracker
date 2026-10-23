// Small helpers for "Start Job Tracker Local.cmd".
// They run in Node so the CMD script avoids fragile quoting and gets
// cryptographically secure random values.
//
//   node deployment/local-tools.mjs secret             -> 64 hex characters (32 random bytes)
//   node deployment/local-tools.mjs timestamp          -> local time as YYYYMMDD-HHMMSS
//   node deployment/local-tools.mjs check-node <major> -> prints the Node version; exit 0 when its major version is at least <major>
import { randomBytes } from "node:crypto";

const pad = (value) => String(value).padStart(2, "0");

const commands = {
  secret: () => { console.log(randomBytes(32).toString("hex")); return 0; },
  timestamp: () => {
    const now = new Date();
    console.log(`${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`);
    return 0;
  },
  "check-node": (minimumText) => {
    const minimum = Number.parseInt(minimumText ?? "", 10);
    if (!Number.isInteger(minimum)) { console.error("check-node needs a minimum major version"); return 2; }
    const major = Number.parseInt(process.versions.node.split(".")[0], 10);
    console.log(process.versions.node);
    return major >= minimum ? 0 : 1;
  },
};

const [name, argument] = process.argv.slice(2);
const command = commands[name];
if (!command) {
  console.error("Usage: node deployment/local-tools.mjs secret | timestamp | check-node <major>");
  process.exitCode = 2;
} else {
  process.exitCode = command(argument);
}
