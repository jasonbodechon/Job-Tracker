@echo off
setlocal EnableExtensions EnableDelayedExpansion
title Job Tracker - Local Docker
cd /d "%~dp0"

rem Usage:
rem   "Start Job Tracker Local.cmd"         Full run: install, test, rebuild, back up, upgrade and start.
rem   "Start Job Tracker Local.cmd" quick   Start the existing containers without testing or rebuilding.
set "JT_QUICK="
if /i "%~1"=="quick" set "JT_QUICK=1"
set "JT_COMPOSE=docker compose --env-file .env.local -f docker-compose.yml -f docker-compose.local.yml"
set "JT_MIN_NODE_MAJOR=20"

call :ensure_docker
if errorlevel 1 goto :fail
call :ensure_node
if errorlevel 1 goto :fail
call :ensure_local_environment
if errorlevel 1 goto :fail
call :resolve_install_mode
if errorlevel 1 goto :fail
if defined JT_QUICK if "!DEPLOYMENT_MODE!"=="NEW INSTALL" (
  echo Quick start needs an existing installation. Running the full setup instead.
  set "JT_QUICK="
)
if defined JT_QUICK (echo Job Tracker mode: QUICK START) else echo Job Tracker mode: !DEPLOYMENT_MODE!
if defined JT_QUICK goto :start_services

call :step "Installing locked frontend dependencies"
if exist "package-lock.json" (call npm ci) else (call npm install)
if errorlevel 1 (set "FAIL_MESSAGE=Frontend dependencies could not be installed." & goto :fail)
call :security_audit
call :step "Running TypeScript and frontend unit tests"
call npm run test:frontend
if errorlevel 1 (set "FAIL_MESSAGE=TypeScript and frontend unit tests failed." & goto :fail)

call :step "Downloading updated PostgreSQL and Caddy images"
%JT_COMPOSE% pull postgres caddy
if errorlevel 1 (set "FAIL_MESSAGE=Updated PostgreSQL or Caddy images could not be downloaded." & goto :fail)

rem The Docker build compiles the production frontend and runs the Rust server tests.
call :step "Building and testing the updated Docker application images"
%JT_COMPOSE% --progress=plain build --pull
if errorlevel 1 (set "FAIL_MESSAGE=Docker build or Rust server tests failed." & goto :fail)

call :backup_database
if errorlevel 1 goto :fail

:start_services
call :release_local_port
if errorlevel 1 goto :fail

call :step "Installing or upgrading the running local Job Tracker containers"
%JT_COMPOSE% up -d --remove-orphans
if errorlevel 1 (set "FAIL_MESSAGE=The local Docker services could not be installed or upgraded." & goto :fail)

call :sync_local_database_password
if errorlevel 1 goto :fail

call :wait_for_url "http://localhost:8080/api/health"
if errorlevel 1 (set "FAIL_MESSAGE=Job Tracker did not become healthy at http://localhost:8080." & goto :fail)

start "" "http://localhost:8080"
echo.
echo Job Tracker is running locally at http://localhost:8080
if defined JT_QUICK (echo Completed mode: QUICK START) else echo Completed mode: !DEPLOYMENT_MODE!
if defined JT_BACKUP_FILE echo Database backup saved to !JT_BACKUP_FILE!
echo To stop it later, run: %JT_COMPOSE% down
goto :success

:ensure_docker
set "JT_DOCKER_JUST_INSTALLED="
where docker.exe >nul 2>&1
if not errorlevel 1 goto :docker_installed
where winget.exe >nul 2>&1
if errorlevel 1 (set "FAIL_MESSAGE=Windows Package Manager is required to install Docker Desktop." & exit /b 1)
call :step "Installing Docker Desktop"
winget install --id Docker.DockerDesktop --exact --accept-package-agreements --accept-source-agreements
if errorlevel 1 (set "FAIL_MESSAGE=Docker Desktop could not be installed." & exit /b 1)
set "JT_DOCKER_JUST_INSTALLED=1"
set "PATH=C:\Program Files\Docker\Docker\resources\bin;%PATH%"
:docker_installed
docker info >nul 2>&1
if not errorlevel 1 goto :docker_ready
if exist "C:\Program Files\Docker\Docker\Docker Desktop.exe" start "" "C:\Program Files\Docker\Docker\Docker Desktop.exe"
call :step "Waiting for Docker Desktop to start"
call :wait_for_docker
if not errorlevel 1 goto :docker_ready
if defined JT_DOCKER_JUST_INSTALLED (
  set "FAIL_MESSAGE=Docker Desktop was just installed and needs a Windows restart to finish setting up. Restart Windows, open Docker Desktop once to accept its terms, then run this file again."
) else (
  set "FAIL_MESSAGE=Docker Desktop is installed but did not start within 2 minutes. Start it, wait until it shows Engine running, then run this file again."
)
exit /b 1
:docker_ready
docker compose version >nul 2>&1
if errorlevel 1 (set "FAIL_MESSAGE=Docker Compose V2 is required." & exit /b 1)
exit /b 0

:wait_for_docker
set /a DOCKER_ATTEMPTS=0
:wait_for_docker_loop
docker info >nul 2>&1
if not errorlevel 1 exit /b 0
set /a DOCKER_ATTEMPTS+=1
if !DOCKER_ATTEMPTS! geq 24 exit /b 1
timeout /t 5 /nobreak >nul
goto :wait_for_docker_loop

:ensure_node
where npm.cmd >nul 2>&1
if not errorlevel 1 goto :check_node_version
where winget.exe >nul 2>&1
if errorlevel 1 (set "FAIL_MESSAGE=Node.js LTS is required and Windows Package Manager is unavailable." & exit /b 1)
call :step "Installing Node.js LTS"
winget install --id OpenJS.NodeJS.LTS --exact --accept-package-agreements --accept-source-agreements
if errorlevel 1 (set "FAIL_MESSAGE=Node.js LTS could not be installed." & exit /b 1)
set "PATH=C:\Program Files\nodejs;%PATH%"
where npm.cmd >nul 2>&1
if errorlevel 1 (set "FAIL_MESSAGE=Node.js was installed but npm is not available. Restart Windows, then run this file again." & exit /b 1)
:check_node_version
set "JT_NODE_VERSION="
for /f "usebackq delims=" %%V in (`node deployment\local-tools.mjs check-node %JT_MIN_NODE_MAJOR%`) do set "JT_NODE_VERSION=%%V"
node deployment\local-tools.mjs check-node %JT_MIN_NODE_MAJOR% >nul 2>&1
if errorlevel 1 (
  set "FAIL_MESSAGE=Node.js !JT_NODE_VERSION! is too old. Job Tracker needs Node.js %JT_MIN_NODE_MAJOR% or newer. Update it with: winget upgrade --id OpenJS.NodeJS.LTS --exact  then open a new window and run this file again."
  exit /b 1
)
exit /b 0

:ensure_local_environment
if exist ".env.local" exit /b 0
call :step "Creating local-only Docker settings"
set "JT_NEW_DB_PASSWORD="
set "JT_NEW_ENCRYPTION_KEY="
for /f "usebackq delims=" %%S in (`node deployment\local-tools.mjs secret`) do set "JT_NEW_DB_PASSWORD=%%S"
for /f "usebackq delims=" %%S in (`node deployment\local-tools.mjs secret`) do set "JT_NEW_ENCRYPTION_KEY=%%S"
if not defined JT_NEW_DB_PASSWORD (set "FAIL_MESSAGE=Secure local secrets could not be generated with Node.js." & exit /b 1)
if not defined JT_NEW_ENCRYPTION_KEY (set "FAIL_MESSAGE=Secure local secrets could not be generated with Node.js." & exit /b 1)
(
  echo POSTGRES_PASSWORD=!JT_NEW_DB_PASSWORD!
  echo JOB_TRACKER_ENV_FILE=.env.local
  echo ENCRYPTION_KEY=!JT_NEW_ENCRYPTION_KEY!
  echo APP_ORIGIN=http://localhost:8080
  echo JOB_TRACKER_DOMAIN=localhost
  echo COOKIE_SECURE=false
  echo SESSION_TTL_DAYS=30
  echo WORKER_POLL_SECONDS=60
  echo CONNECTOR_MAX_RESULTS=5000
  echo RUST_LOG=job_tracker_web=info,tower_http=info
  echo VAPID_PUBLIC_KEY=
  echo VAPID_PRIVATE_KEY=
  echo VAPID_SUBJECT=mailto:local@localhost
  echo ADZUNA_APP_ID=
  echo ADZUNA_APP_KEY=
) > ".env.local"
set "JT_NEW_DB_PASSWORD="
set "JT_NEW_ENCRYPTION_KEY="
echo Created .env.local for this PC only. Do not use it for production.
exit /b 0

:backup_database
set "JT_BACKUP_FILE="
if not defined JT_HAS_DATABASE exit /b 0
call :step "Backing up the local database before upgrading"
%JT_COMPOSE% up -d --wait postgres
if errorlevel 1 (set "FAIL_MESSAGE=PostgreSQL could not be started to back up the database. Nothing was upgraded." & exit /b 1)
if not exist "backups" mkdir "backups"
set "JT_BACKUP_STAMP="
for /f "usebackq delims=" %%T in (`node deployment\local-tools.mjs timestamp`) do set "JT_BACKUP_STAMP=%%T"
if not defined JT_BACKUP_STAMP (set "FAIL_MESSAGE=A backup file name could not be created. Nothing was upgraded." & exit /b 1)
set "JT_BACKUP_PATH=backups\job-tracker-!JT_BACKUP_STAMP!.dump"
%JT_COMPOSE% exec -T postgres pg_dump -U job_tracker -d job_tracker -Fc > "!JT_BACKUP_PATH!"
if errorlevel 1 goto :backup_failed
for %%F in ("!JT_BACKUP_PATH!") do if %%~zF lss 1 goto :backup_failed
set "JT_BACKUP_FILE=!JT_BACKUP_PATH!"
echo Saved !JT_BACKUP_FILE!
exit /b 0
:backup_failed
if exist "!JT_BACKUP_PATH!" del /q "!JT_BACKUP_PATH!"
set "FAIL_MESSAGE=The database backup failed, so nothing was upgraded. Your existing data has not been changed."
exit /b 1

:sync_local_database_password
rem The password is passed as a psql variable and quoted by psql itself, so quotes
rem and backslashes in it cannot break the SQL. CMD itself cannot carry a double
rem quote or an exclamation mark, so keep those out of POSTGRES_PASSWORD.
set "JT_POSTGRES_PASSWORD="
for /f "usebackq tokens=1,* delims==" %%A in (".env.local") do if /i "%%A"=="POSTGRES_PASSWORD" set "JT_POSTGRES_PASSWORD=%%B"
if not defined JT_POSTGRES_PASSWORD (set "FAIL_MESSAGE=POSTGRES_PASSWORD is missing from .env.local." & exit /b 1)
call :step "Synchronizing local database credentials"
set "JT_SYNC_SQL=%TEMP%\job-tracker-sync-!RANDOM!!RANDOM!.sql"
> "!JT_SYNC_SQL!" echo ALTER ROLE job_tracker WITH PASSWORD :'jt_password';
%JT_COMPOSE% exec -T postgres psql -q -v ON_ERROR_STOP=1 -v "jt_password=!JT_POSTGRES_PASSWORD!" -U job_tracker -d job_tracker < "!JT_SYNC_SQL!"
set "JT_SYNC_RESULT=!ERRORLEVEL!"
del /q "!JT_SYNC_SQL!" >nul 2>&1
set "JT_POSTGRES_PASSWORD="
if not "!JT_SYNC_RESULT!"=="0" (set "FAIL_MESSAGE=The local PostgreSQL password could not be synchronized. Run the script again after Docker finishes starting." & exit /b 1)
exit /b 0

:resolve_install_mode
set "JT_REQUESTED_PROJECT=!COMPOSE_PROJECT_NAME!"
if not defined JT_REQUESTED_PROJECT for /f "tokens=1,* delims==" %%A in ('findstr /b /c:"COMPOSE_PROJECT_NAME=" ".env.local" 2^>nul') do set "JT_REQUESTED_PROJECT=%%B"
set "COMPOSE_PROJECT_NAME="
set "JT_EXISTING_PROJECT="
set "JT_EXISTING_VOLUME="
set "JT_HAS_DATABASE="
set /a JT_PROJECT_COUNT=0
if defined JT_REQUESTED_PROJECT (
  set "COMPOSE_PROJECT_NAME=!JT_REQUESTED_PROJECT!"
  goto :classify_local_install_mode
)
for /f "delims=" %%V in ('docker volume ls --format "{{.Name}}" ^| findstr /r /i "_job_tracker_postgres$"') do (
  set /a JT_PROJECT_COUNT+=1
  set "JT_EXISTING_VOLUME=%%V"
)
if !JT_PROJECT_COUNT! gtr 1 (
  set "FAIL_MESSAGE=Multiple Job Tracker PostgreSQL volumes were found. To protect your data, remove unused test instances or set COMPOSE_PROJECT_NAME before retrying."
  exit /b 1
)
if !JT_PROJECT_COUNT! equ 1 set "JT_EXISTING_PROJECT=!JT_EXISTING_VOLUME:_job_tracker_postgres=!"
if defined JT_EXISTING_PROJECT (set "COMPOSE_PROJECT_NAME=!JT_EXISTING_PROJECT!") else set "COMPOSE_PROJECT_NAME=job-tracker"
:classify_local_install_mode
set "DEPLOYMENT_MODE=NEW INSTALL"
for /f "delims=" %%C in ('%JT_COMPOSE% ps -a -q api 2^>nul') do set "DEPLOYMENT_MODE=UPDATE"
for /f "delims=" %%C in ('%JT_COMPOSE% ps -a -q worker 2^>nul') do set "DEPLOYMENT_MODE=UPDATE"
docker volume inspect "!COMPOSE_PROJECT_NAME!_job_tracker_postgres" >nul 2>&1 && set "DEPLOYMENT_MODE=UPDATE" && set "JT_HAS_DATABASE=1"
docker volume inspect "!COMPOSE_PROJECT_NAME!_caddy_data" >nul 2>&1 && set "DEPLOYMENT_MODE=UPDATE"
exit /b 0

:release_local_port
set "JT_PORT_BLOCKER="
for /f "delims=" %%C in ('docker ps --filter "publish=8080" --format "{{.Names}}"') do (
  echo %%C | findstr /i /c:"caddy" >nul
  if not errorlevel 1 (
    echo %%C | findstr /i /c:"job-tracker" /c:"jobtracker" >nul
    if not errorlevel 1 (
      call :step "Stopping previous Job-Tracker proxy %%C"
      docker stop "%%C" >nul
      if errorlevel 1 (set "FAIL_MESSAGE=The previous Job-Tracker proxy %%C could not be stopped." & exit /b 1)
    ) else set "JT_PORT_BLOCKER=%%C"
  ) else set "JT_PORT_BLOCKER=%%C"
)
if defined JT_PORT_BLOCKER (
  set "FAIL_MESSAGE=Port 8080 is used by !JT_PORT_BLOCKER!, which is not a Job-Tracker proxy. Stop that application or change its port, then retry."
  exit /b 1
)
exit /b 0

:security_audit
rem Report only. Startup never changes package.json or package-lock.json.
call :step "Checking package security"
call npm audit --audit-level=high
if not errorlevel 1 exit /b 0
echo.
echo WARNING: npm audit reported high or critical package issues, listed above.
echo Job Tracker will still start. Review and update those packages yourself,
echo then commit the changes to package.json and package-lock.json.
exit /b 0

:wait_for_url
set /a URL_ATTEMPTS=0
:wait_for_url_loop
curl.exe --fail --silent "%~1" >nul 2>&1 && exit /b 0
timeout /t 3 /nobreak >nul
set /a URL_ATTEMPTS+=1
if !URL_ATTEMPTS! lss 30 goto :wait_for_url_loop
exit /b 1

:step
echo.
echo ==^> %~1
exit /b 0

:success
echo.
echo Job Tracker startup completed successfully.
echo Press any key to close.
pause >nul
exit /b 0

:fail
echo.
echo ERROR: !FAIL_MESSAGE!
echo Job Tracker was not started. Review the error above.
echo Press any key to close.
pause >nul
exit /b 1
