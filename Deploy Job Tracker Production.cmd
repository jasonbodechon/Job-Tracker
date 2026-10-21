@echo off
setlocal EnableExtensions EnableDelayedExpansion
title Job Tracker - Production Docker Deployment
cd /d "%~dp0"

call :ensure_docker
if errorlevel 1 goto :fail
call :ensure_node
if errorlevel 1 goto :fail
call :validate_production_environment
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
docker compose --env-file .env pull postgres caddy
if errorlevel 1 (set "FAIL_MESSAGE=Updated PostgreSQL or Caddy images could not be downloaded." & goto :fail)

call :step "Building and testing the updated production Docker images"
docker compose --progress=plain --env-file .env build --pull
if errorlevel 1 (set "FAIL_MESSAGE=Docker build or Rust server tests failed." & goto :fail)

call :step "Installing or upgrading the running production Job Tracker containers"
docker compose --env-file .env up -d --remove-orphans
if errorlevel 1 (set "FAIL_MESSAGE=The production Docker services could not be installed or upgraded." & goto :fail)

call :wait_for_url "https://!PUBLIC_URL!/api/health"
if errorlevel 1 (set "FAIL_MESSAGE=Job Tracker did not become healthy over trusted HTTPS. Check DNS, ports 80/443, and docker compose logs." & goto :fail)

start "" "https://!PUBLIC_URL!"
echo.
echo Job Tracker is running at https://!PUBLIC_URL!
echo Completed mode: !DEPLOYMENT_MODE!
echo Caddy manages the trusted HTTPS certificate automatically.
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

:validate_production_environment
if not exist ".env" (
  copy /y ".env.example" ".env" >nul
  start "" notepad.exe ".env"
  set "FAIL_MESSAGE=.env was created. Enter your production domain and strong secrets, save it, then run this deployment file again."
  exit /b 1
)
findstr /b /c:"POSTGRES_PASSWORD=replace-with" ".env" >nul && (set "FAIL_MESSAGE=Replace POSTGRES_PASSWORD in .env with a strong secret." & exit /b 1)
findstr /b /c:"ENCRYPTION_KEY=replace-with" ".env" >nul && (set "FAIL_MESSAGE=Replace ENCRYPTION_KEY in .env with an independent strong secret." & exit /b 1)
findstr /b /c:"JOB_TRACKER_DOMAIN=jobs.example.com" ".env" >nul && (set "FAIL_MESSAGE=Set JOB_TRACKER_DOMAIN to your public DNS name." & exit /b 1)
findstr /b /c:"APP_ORIGIN=https://jobs.example.com" ".env" >nul && (set "FAIL_MESSAGE=Set APP_ORIGIN to your public HTTPS URL." & exit /b 1)
findstr /b /c:"COOKIE_SECURE=true" ".env" >nul || (set "FAIL_MESSAGE=COOKIE_SECURE must be true for production." & exit /b 1)
for /f "tokens=1,* delims==" %%A in ('findstr /b /c:"JOB_TRACKER_DOMAIN=" ".env"') do set "PUBLIC_URL=%%B"
if not defined PUBLIC_URL (set "FAIL_MESSAGE=JOB_TRACKER_DOMAIN is required in .env." & exit /b 1)
echo !PUBLIC_URL! | findstr /i /c:"localhost" /c:"127.0.0.1" >nul && (set "FAIL_MESSAGE=JOB_TRACKER_DOMAIN must be a public DNS name, not localhost." & exit /b 1)
findstr /b /c:"APP_ORIGIN=https://!PUBLIC_URL!" ".env" >nul || (set "FAIL_MESSAGE=APP_ORIGIN must exactly equal https://JOB_TRACKER_DOMAIN." & exit /b 1)
exit /b 0

:resolve_install_mode
set "JT_REQUESTED_PROJECT=!COMPOSE_PROJECT_NAME!"
if not defined JT_REQUESTED_PROJECT for /f "tokens=1,* delims==" %%A in ('findstr /b /c:"COMPOSE_PROJECT_NAME=" ".env" 2^>nul') do set "JT_REQUESTED_PROJECT=%%B"
set "COMPOSE_PROJECT_NAME="
set "JT_EXISTING_PROJECT="
set "JT_EXISTING_VOLUME="
set /a JT_PROJECT_COUNT=0
if defined JT_REQUESTED_PROJECT (
  set "COMPOSE_PROJECT_NAME=!JT_REQUESTED_PROJECT!"
  goto :classify_production_install_mode
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
:classify_production_install_mode
set "DEPLOYMENT_MODE=NEW INSTALL"
for /f "delims=" %%C in ('docker compose --env-file .env ps -a -q api 2^>nul') do set "DEPLOYMENT_MODE=UPDATE"
for /f "delims=" %%C in ('docker compose --env-file .env ps -a -q worker 2^>nul') do set "DEPLOYMENT_MODE=UPDATE"
docker volume inspect "!COMPOSE_PROJECT_NAME!_job_tracker_postgres" >nul 2>&1 && set "DEPLOYMENT_MODE=UPDATE"
docker volume inspect "!COMPOSE_PROJECT_NAME!_caddy_data" >nul 2>&1 && set "DEPLOYMENT_MODE=UPDATE"
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
timeout /t 5 /nobreak >nul
set /a URL_ATTEMPTS+=1
if !URL_ATTEMPTS! lss 36 goto :wait_for_url_loop
exit /b 1

:step
echo.
echo ==^> %~1
exit /b 0

:success
echo.
echo Job Tracker production deployment completed successfully.
echo Press any key to close.
pause >nul
exit /b 0

:fail
echo.
echo ERROR: !FAIL_MESSAGE!
echo Job Tracker was not deployed. Review the error above.
echo Press any key to close.
pause >nul
exit /b 1
