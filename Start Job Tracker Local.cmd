@echo off
setlocal EnableExtensions EnableDelayedExpansion
title Job Tracker - Local Docker
cd /d "%~dp0"

call :ensure_docker
if errorlevel 1 goto :fail
call :ensure_node
if errorlevel 1 goto :fail
call :ensure_local_environment
if errorlevel 1 goto :fail
call :resolve_install_mode
if errorlevel 1 goto :fail
echo Job Tracker mode: !DEPLOYMENT_MODE!
call :step "Installing locked frontend dependencies"
if exist "package-lock.json" (call npm ci) else (call npm install)
if errorlevel 1 (set "FAIL_MESSAGE=Frontend dependencies could not be installed." & goto :fail)
call :security_audit
if errorlevel 1 (set "FAIL_MESSAGE=The frontend security audit could not be repaired." & goto :fail)
call :step "Running TypeScript and frontend unit tests"
call npm run test:frontend
if errorlevel 1 (set "FAIL_MESSAGE=TypeScript and frontend unit tests failed." & goto :fail)
call :step "Building the production PWA"
call npm run build
if errorlevel 1 (set "FAIL_MESSAGE=The production frontend build failed." & goto :fail)

call :step "Downloading updated PostgreSQL and Caddy images"
docker compose --env-file .env.local -f docker-compose.yml -f docker-compose.local.yml pull postgres caddy
if errorlevel 1 (set "FAIL_MESSAGE=Updated PostgreSQL or Caddy images could not be downloaded." & goto :fail)

call :step "Building and testing the updated Docker application images"
docker compose --progress=plain --env-file .env.local -f docker-compose.yml -f docker-compose.local.yml build --pull
if errorlevel 1 (set "FAIL_MESSAGE=Docker build or Rust server tests failed." & goto :fail)

call :release_local_port
if errorlevel 1 goto :fail

call :step "Installing or upgrading the running local Job Tracker containers"
docker compose --env-file .env.local -f docker-compose.yml -f docker-compose.local.yml up -d --remove-orphans
if errorlevel 1 (set "FAIL_MESSAGE=The local Docker services could not be installed or upgraded." & goto :fail)

call :sync_local_database_password
if errorlevel 1 (set "FAIL_MESSAGE=The local PostgreSQL password could not be synchronized. Run the script again after Docker finishes starting." & goto :fail)

call :wait_for_url "http://localhost:8080/api/health"
if errorlevel 1 (set "FAIL_MESSAGE=Job Tracker did not become healthy at http://localhost:8080." & goto :fail)

start "" "http://localhost:8080"
echo.
echo Job Tracker is running locally at http://localhost:8080
echo Completed mode: !DEPLOYMENT_MODE!
echo To stop it later, run: docker compose --env-file .env.local -f docker-compose.yml -f docker-compose.local.yml down
goto :success

:ensure_docker
where docker.exe >nul 2>&1
if errorlevel 1 (
  where winget.exe >nul 2>&1
  if errorlevel 1 (set "FAIL_MESSAGE=Windows Package Manager is required to install Docker Desktop." & exit /b 1)
  call :step "Installing Docker Desktop"
  winget install --id Docker.DockerDesktop --exact --accept-package-agreements --accept-source-agreements
  if errorlevel 1 exit /b 1
  set "PATH=C:\Program Files\Docker\Docker\resources\bin;%PATH%"
)
docker version >nul 2>&1
if errorlevel 1 (
  if exist "C:\Program Files\Docker\Docker\Docker Desktop.exe" start "" "C:\Program Files\Docker\Docker\Docker Desktop.exe"
  call :step "Waiting for Docker Desktop to start"
  set /a DOCKER_ATTEMPTS=0
  :wait_for_docker
  timeout /t 5 /nobreak >nul
  docker info >nul 2>&1 && goto :docker_ready
  set /a DOCKER_ATTEMPTS+=1
  if !DOCKER_ATTEMPTS! lss 24 goto :wait_for_docker
  set "FAIL_MESSAGE=Docker Desktop is installed but is not running. Start it, wait until it is ready, then run this file again."
  exit /b 1
)
:docker_ready
docker compose version >nul 2>&1
if errorlevel 1 (set "FAIL_MESSAGE=Docker Compose V2 is required." & exit /b 1)
exit /b 0

:ensure_node
where npm.cmd >nul 2>&1
if errorlevel 1 (
  where winget.exe >nul 2>&1
  if errorlevel 1 (set "FAIL_MESSAGE=Node.js LTS is required and Windows Package Manager is unavailable." & exit /b 1)
  call :step "Installing Node.js LTS"
  winget install --id OpenJS.NodeJS.LTS --exact --accept-package-agreements --accept-source-agreements
  if errorlevel 1 exit /b 1
  set "PATH=C:\Program Files\nodejs;%PATH%"
)
where npm.cmd >nul 2>&1
if errorlevel 1 (set "FAIL_MESSAGE=Node.js was installed but npm is not available. Restart Windows, then run this file again." & exit /b 1)
exit /b 0

:ensure_local_environment
if exist ".env.local" exit /b 0
call :step "Creating local-only Docker settings"
(
  echo POSTGRES_PASSWORD=local-!RANDOM!-!RANDOM!-!RANDOM!
  echo JOB_TRACKER_ENV_FILE=.env.local
  echo ENCRYPTION_KEY=local-!RANDOM!-!RANDOM!-!RANDOM!-!RANDOM!
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
echo Created .env.local for this PC only. Do not use it for production.
exit /b 0

:sync_local_database_password
set "JT_POSTGRES_PASSWORD="
for /f "usebackq tokens=1,* delims==" %%A in (".env.local") do if /i "%%A"=="POSTGRES_PASSWORD" set "JT_POSTGRES_PASSWORD=%%B"
if not defined JT_POSTGRES_PASSWORD exit /b 1
call :step "Synchronizing local database credentials"
docker compose --env-file .env.local -f docker-compose.yml -f docker-compose.local.yml exec -T postgres psql -v ON_ERROR_STOP=1 -U job_tracker -d job_tracker -c "ALTER ROLE job_tracker WITH PASSWORD '!JT_POSTGRES_PASSWORD!';"
exit /b %ERRORLEVEL%

:resolve_install_mode
set "JT_REQUESTED_PROJECT=!COMPOSE_PROJECT_NAME!"
if not defined JT_REQUESTED_PROJECT for /f "tokens=1,* delims==" %%A in ('findstr /b /c:"COMPOSE_PROJECT_NAME=" ".env.local" 2^>nul') do set "JT_REQUESTED_PROJECT=%%B"
set "COMPOSE_PROJECT_NAME="
set "JT_EXISTING_PROJECT="
set "JT_EXISTING_VOLUME="
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
for /f "delims=" %%C in ('docker compose --env-file .env.local -f docker-compose.yml -f docker-compose.local.yml ps -a -q api 2^>nul') do set "DEPLOYMENT_MODE=UPDATE"
for /f "delims=" %%C in ('docker compose --env-file .env.local -f docker-compose.yml -f docker-compose.local.yml ps -a -q worker 2^>nul') do set "DEPLOYMENT_MODE=UPDATE"
docker volume inspect "!COMPOSE_PROJECT_NAME!_job_tracker_postgres" >nul 2>&1 && set "DEPLOYMENT_MODE=UPDATE"
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
call :step "Checking package security"
call npm audit --audit-level=high
if not errorlevel 1 exit /b 0
echo High or critical package issues were found. Running npm audit fix --force...
call npm audit fix --force
if errorlevel 1 exit /b 1
call npm install
if errorlevel 1 exit /b 1
call npm audit --audit-level=high
exit /b %ERRORLEVEL%

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
