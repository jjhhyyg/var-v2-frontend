import { chmodSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const __dirname = dirname(fileURLToPath(import.meta.url))
const frontendDir = resolve(__dirname, '..')
const rootDir = resolve(frontendDir, '..')
const aiDir = join(rootDir, 'ai-processor')
const resourcesDir = join(frontendDir, 'src-tauri', 'resources')
const runtimeResourcesDir = join(resourcesDir, 'runtime')
const platformRuntimeDir = join(runtimeResourcesDir, platformSlug())
const workerResourcesDir = join(platformRuntimeDir, 'worker')
const modelResourcesDir = join(resourcesDir, 'models')
const toolResourcesDir = join(platformRuntimeDir, 'tools')
const legacyWorkerResourcesDir = join(resourcesDir, 'worker')
const legacyBinResourcesDir = join(resourcesDir, 'bin')
const pyInstallerRoot = join(frontendDir, '.tauri-worker-build')
const workerVenvDir = join(frontendDir, '.desktop-worker-venv')
const workerBuildStampPath = join(pyInstallerRoot, 'worker-build-stamp.json')

function platformSlug() {
  const platform = process.platform === 'darwin'
    ? 'darwin'
    : process.platform === 'win32'
      ? 'windows'
      : process.platform
  const arch = process.arch === 'x64'
    ? 'x64'
    : process.arch === 'arm64'
      ? 'arm64'
      : process.arch
  return `${platform}-${arch}`
}

function bundledExecutableName(name) {
  return process.platform === 'win32' ? `${name}.exe` : name
}

function runCommand(cmd, args, options = {}) {
  const result = spawnSync(cmd, args, {
    stdio: 'inherit',
    ...options
  })

  if (result.status !== 0) {
    throw new Error(`命令执行失败: ${cmd} ${args.join(' ')}`)
  }
}

function verifyPackagedWorker(distDir) {
  const executable = join(distDir, bundledExecutableName('desktop_worker'))
  runCommand(executable, ['--self-check'], {
    cwd: aiDir,
    env: {
      ...process.env,
      ULTRALYTICS_SKIP_REQUIREMENTS_CHECKS: '1'
    }
  })
}

function ensureDir(path) {
  mkdirSync(path, { recursive: true })
}

function resolvePythonCommand() {
  const candidates = ['python3.12', 'python3', 'python']

  for (const candidate of candidates) {
    if (requireCommand(candidate)) {
      return candidate
    }
  }

  throw new Error('未找到可用的 Python 解释器，至少需要 python3.12 或 python3')
}

function requireCommand(cmd, args = ['--version']) {
  const result = spawnSync(cmd, args, { stdio: 'pipe' })
  return result.status === 0
}

function copyFileIfExists(source, targetDir, targetName) {
  if (!existsSync(source)) {
    throw new Error(`缺少文件: ${source}`)
  }
  ensureDir(targetDir)
  cpSync(realpathSync(source), join(targetDir, targetName))
}

function normalizeBundledResource(targetPath) {
  const stats = statSync(targetPath)
  if (stats.isDirectory()) {
    chmodSync(targetPath, 0o755)
    for (const entry of readdirSync(targetPath)) {
      normalizeBundledResource(join(targetPath, entry))
    }
    return
  }

  chmodSync(targetPath, 0o644)
}

function resolveWhich(binary) {
  const locator = process.platform === 'win32' ? 'where' : 'which'
  const result = spawnSync(locator, [binary], { encoding: 'utf-8' })
  if (result.status !== 0) {
    throw new Error(`未找到系统命令: ${binary}`)
  }
  return result.stdout.trim().split(/\r?\n/)[0]
}

function resolveRequirementsFile() {
  if (process.platform === 'darwin') {
    return join(aiDir, 'requirements-desktop-macos.txt')
  }

  if (process.platform === 'win32') {
    return join(aiDir, 'requirements-desktop-windows-cpu.txt')
  }

  return join(aiDir, 'requirements-cpu.txt')
}

function resolveCondaPythonPath(envName) {
  if (!requireCommand('conda')) {
    return null
  }

  const result = spawnSync(
    'conda',
    ['run', '-n', envName, 'python', '-c', 'import sys; print(sys.executable)'],
    {
      encoding: 'utf-8'
    }
  )

  if (result.status !== 0) {
    return null
  }

  const lines = result.stdout
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(Boolean)

  return lines.at(-1) ?? null
}

function resolveVenvPaths() {
  if (process.platform === 'win32') {
    return {
      python: join(workerVenvDir, 'Scripts', 'python.exe')
    }
  }

  return {
    python: join(workerVenvDir, 'bin', 'python')
  }
}

function modulesReady(python) {
  const check = spawnSync(
    python,
    [
      '-c',
      'import PyInstaller, cv2, torch, ultralytics, dotenv, numpy, scipy, PIL, lap'
    ],
    {
      stdio: 'pipe'
    }
  )

  return check.status === 0
}

function ensureNoBackportPathlib(python) {
  const result = spawnSync(
    python,
    [
      '-c',
      'import sys, glob, os; matches=[]; [matches.extend(glob.glob(os.path.join(path, "pathlib.py"))) for path in sys.path if "site-packages" in path]; print(matches[0] if matches else "")'
    ],
    {
      encoding: 'utf-8'
    }
  )

  if (result.status !== 0) {
    return
  }

  const pathlibBackport = result.stdout.trim()
  if (!pathlibBackport) {
    return
  }

  runCommand(python, ['-m', 'pip', 'uninstall', '-y', 'pathlib'])
}

function latestWorkerSourceMtime(root) {
  let latest = 0
  const ignoredDirs = new Set(['__pycache__', 'logs', 'storage'])
  const includedExtensions = new Set(['.py', '.yaml', '.yml', '.pt', '.json'])

  for (const entry of readdirSync(root, { withFileTypes: true })) {
    const fullPath = join(root, entry.name)

    if (entry.isDirectory()) {
      if (ignoredDirs.has(entry.name)) {
        continue
      }
      latest = Math.max(latest, latestWorkerSourceMtime(fullPath))
      continue
    }

    const extension = entry.name.slice(entry.name.lastIndexOf('.'))
    if (!includedExtensions.has(extension)) {
      continue
    }

    latest = Math.max(latest, Math.floor(statSync(fullPath).mtimeMs))
  }

  return latest
}

function buildInputMtime(requirementsFile) {
  return Math.max(
    latestWorkerSourceMtime(aiDir),
    Math.floor(statSync(requirementsFile).mtimeMs),
    Math.floor(statSync(fileURLToPath(import.meta.url)).mtimeMs)
  )
}

function ensurePythonDependencies({ python, requirementsFile, stampPath, pythonIdentity }) {
  const requirementsMtime = Math.floor(statSync(requirementsFile).mtimeMs)
  ensureNoBackportPathlib(python)

  let stamp = null
  if (existsSync(stampPath)) {
    try {
      stamp = JSON.parse(readFileSync(stampPath, 'utf-8'))
    } catch {
      stamp = null
    }
  }

  let shouldInstall = true
  if (stamp) {
    shouldInstall = !(
      stamp.requirementsFile === requirementsFile
      && stamp.requirementsMtime === requirementsMtime
      && stamp.pythonIdentity === pythonIdentity
      && modulesReady(python)
    )
  } else {
    shouldInstall = !modulesReady(python)
  }

  if (shouldInstall) {
    runCommand(python, ['-m', 'pip', 'install', '--upgrade', 'pip'])
    runCommand(python, ['-m', 'pip', 'install', '-r', requirementsFile, 'pyinstaller>=6.0.0'])
    writeFileSync(stampPath, JSON.stringify({
      pythonIdentity,
      requirementsFile,
      requirementsMtime
    }, null, 2))
  }

  return { python, pythonIdentity }
}

function ensureBuildPython() {
  const requirementsFile = resolveRequirementsFile()
  const condaEnvName = process.env.TAURI_WORKER_CONDA_ENV ?? 'var-env'
  const explicitPython = process.env.TAURI_WORKER_PYTHON

  ensureDir(pyInstallerRoot)

  if (explicitPython && existsSync(explicitPython)) {
    return ensurePythonDependencies({
      python: explicitPython,
      requirementsFile,
      stampPath: join(pyInstallerRoot, 'explicit-python-ready.json'),
      pythonIdentity: `explicit:${explicitPython}`
    })
  }

  const condaPython = resolveCondaPythonPath(condaEnvName)
  if (condaPython && existsSync(condaPython)) {
    return ensurePythonDependencies({
      python: condaPython,
      requirementsFile,
      stampPath: join(pyInstallerRoot, `conda-${condaEnvName}-ready.json`),
      pythonIdentity: `conda:${condaEnvName}:${condaPython}`
    })
  }

  const pythonCommand = resolvePythonCommand()
  const { python } = resolveVenvPaths()
  const stampPath = join(workerVenvDir, '.desktop-worker-ready.json')

  let stamp = null
  if (existsSync(stampPath)) {
    try {
      stamp = JSON.parse(readFileSync(stampPath, 'utf-8'))
    } catch {
      stamp = null
    }
  }

  if (stamp?.pythonIdentity && stamp.pythonIdentity !== `venv:${pythonCommand}`) {
    rmSync(workerVenvDir, { recursive: true, force: true })
  }

  if (!existsSync(python)) {
    runCommand(pythonCommand, ['-m', 'venv', workerVenvDir])
  }

  return ensurePythonDependencies({
    python,
    requirementsFile,
    stampPath,
    pythonIdentity: `venv:${pythonCommand}`
  })
}

function buildWorker() {
  const requirementsFile = resolveRequirementsFile()
  const { python, pythonIdentity } = ensureBuildPython()
  const excludedModules = ['PyQt5', 'PyQt6', 'PySide2', 'PySide6']
  const cachedWorkerDir = join(workerResourcesDir, 'desktop_worker')
  const sourceMtime = buildInputMtime(requirementsFile)

  if (existsSync(workerBuildStampPath) && existsSync(cachedWorkerDir)) {
    try {
      const stamp = JSON.parse(readFileSync(workerBuildStampPath, 'utf-8'))
      if (stamp.pythonIdentity === pythonIdentity && stamp.sourceMtime === sourceMtime) {
        return
      }
    } catch {
      // 忽略缓存损坏，走完整构建
    }
  }

  rmSync(workerResourcesDir, { recursive: true, force: true })
  rmSync(join(pyInstallerRoot, 'dist'), { recursive: true, force: true })
  rmSync(join(pyInstallerRoot, 'build'), { recursive: true, force: true })
  rmSync(join(pyInstallerRoot, 'desktop_worker.spec'), { force: true })
  ensureDir(workerResourcesDir)
  ensureDir(pyInstallerRoot)

  const build = spawnSync(
    python,
    [
      '-m',
      'PyInstaller',
      '--noconfirm',
      '--clean',
      '--onedir',
      '--name',
      'desktop_worker',
      '--distpath',
      join(pyInstallerRoot, 'dist'),
      '--workpath',
      join(pyInstallerRoot, 'build'),
      '--specpath',
      pyInstallerRoot,
      '--paths',
      aiDir,
      '--collect-all',
      'lap',
      ...excludedModules.flatMap(moduleName => ['--exclude-module', moduleName]),
      join(aiDir, 'desktop_worker.py')
    ],
    {
      stdio: 'inherit',
      cwd: aiDir,
      env: {
        ...process.env,
        MPLBACKEND: 'Agg'
      }
    }
  )

  if (build.status !== 0) {
    throw new Error('PyInstaller 构建桌面 worker 失败')
  }

  verifyPackagedWorker(join(pyInstallerRoot, 'dist', 'desktop_worker'))

  cpSync(join(pyInstallerRoot, 'dist', 'desktop_worker'), cachedWorkerDir, {
    recursive: true,
    dereference: true
  })
  normalizeBundledResource(cachedWorkerDir)
  writeFileSync(workerBuildStampPath, JSON.stringify({
    pythonIdentity,
    sourceMtime
  }, null, 2))
}

function copyRuntimeResources() {
  rmSync(modelResourcesDir, { recursive: true, force: true })
  rmSync(toolResourcesDir, { recursive: true, force: true })
  rmSync(legacyBinResourcesDir, { recursive: true, force: true })
  rmSync(legacyWorkerResourcesDir, { recursive: true, force: true })
  ensureDir(modelResourcesDir)
  ensureDir(toolResourcesDir)

  const ffmpegBinaryName = process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg'
  const ffprobeBinaryName = process.platform === 'win32' ? 'ffprobe.exe' : 'ffprobe'

  copyFileIfExists(join(aiDir, 'weights', 'best.pt'), modelResourcesDir, 'best.pt')
  copyFileIfExists(resolveWhich('ffmpeg'), toolResourcesDir, ffmpegBinaryName)
  copyFileIfExists(resolveWhich('ffprobe'), toolResourcesDir, ffprobeBinaryName)
  normalizeBundledResource(toolResourcesDir)
}

function main() {
  copyRuntimeResources()
  buildWorker()
  console.log('桌面 worker 和运行资源已更新到 src-tauri/resources')
}

main()
