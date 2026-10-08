@echo off
setlocal EnableExtensions EnableDelayedExpansion

rem Commit local changes and push the current branch to GitHub.
rem Usage:
rem   commit_and_push.bat
rem   commit_and_push.bat "Commit message here"
rem
rem First time only, set ORIGIN_URL below or pass it as second argument:
rem   commit_and_push.bat "Initial push" https://github.com/USER/REPO.git

cd /d "%~dp0"

set "COMMIT_MSG=%~1"
set "ORIGIN_URL=%~2"
if "%ORIGIN_URL%"=="" set "ORIGIN_URL="

where git >nul 2>&1
if errorlevel 1 (
  echo ERROR: git was not found in PATH.
  exit /b 1
)

if not exist ".git" (
  echo ERROR: this folder is not a git repository.
  echo Run "git init" first, or ask Cursor to create the initial commit.
  exit /b 1
)

echo.
echo === Git status ===
git status --short --branch
if errorlevel 1 exit /b 1

echo.
echo Ignored directories that must never be published: target\  secrets\
git check-ignore -q secrets 2>nul
if not errorlevel 1 (
  echo secrets\ is ignored. OK.
) else (
  echo WARNING: secrets\ does not appear to be ignored. Aborting.
  exit /b 1
)

rem Configure origin if missing.
git remote get-url origin >nul 2>&1
if errorlevel 1 (
  if "%ORIGIN_URL%"=="" (
    echo.
    echo No GitHub remote named "origin" is configured yet.
    set /p ORIGIN_URL=Paste the GitHub repo URL ^(https://github.com/USER/REPO.git^): 
  )
  if "!ORIGIN_URL!"=="" (
    echo ERROR: no remote URL provided.
    exit /b 1
  )
  echo.
  echo Adding origin: !ORIGIN_URL!
  git remote add origin "!ORIGIN_URL!"
  if errorlevel 1 exit /b 1
) else (
  for /f "delims=" %%U in ('git remote get-url origin') do echo Remote origin: %%U
)

rem Detect current branch.
for /f "delims=" %%B in ('git rev-parse --abbrev-ref HEAD') do set "BRANCH=%%B"
if "%BRANCH%"=="" (
  echo ERROR: could not detect the current branch.
  exit /b 1
)
echo Branch: %BRANCH%

rem Stage all tracked/untracked changes that are not ignored.
echo.
echo === Staging changes ===
git add -A
if errorlevel 1 exit /b 1

rem Refuse to commit if secrets somehow got staged.
git diff --cached --name-only | findstr /I /R "^secrets[/\\]" >nul
if not errorlevel 1 (
  echo ERROR: files under secrets\ are staged. Unstage them and check .gitignore.
  git reset
  exit /b 1
)

git diff --cached --quiet
if not errorlevel 1 (
  echo.
  echo Nothing new to commit. Working tree is clean.
) else (
  if "%COMMIT_MSG%"=="" (
    echo.
    set /p COMMIT_MSG=Commit message: 
  )
  if "!COMMIT_MSG!"=="" (
    echo ERROR: empty commit message.
    exit /b 1
  )

  echo.
  echo === Creating commit ===
  git commit -m "!COMMIT_MSG!"
  if errorlevel 1 exit /b 1
)

echo.
echo === Pushing to GitHub ===
git rev-parse --abbrev-ref --symbolic-full-name "@{u}" >nul 2>&1
if errorlevel 1 (
  git push -u origin "%BRANCH%"
) else (
  git push
)
if errorlevel 1 (
  echo.
  echo Push failed. Common fixes:
  echo   1. Create the empty repository on GitHub first
  echo   2. Sign in with GitHub CLI: gh auth login
  echo   3. Or use a personal access token when git asks for a password
  exit /b 1
)

echo.
echo Done.
git status --short --branch
exit /b 0
