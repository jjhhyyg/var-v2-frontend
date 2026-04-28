import { chmodSync, cpSync, existsSync, mkdirSync, readFileSync, readdirSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import { copyPeDependencies } from './pe-deps.mjs'

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
const workerPyInstallerSpecPath = join(pyInstallerRoot, 'desktop_worker.spec')
const sourceOnnxModelPath = join(aiDir, 'weights', 'best.onnx')
const workerPyInstallerExcludes = Object.freeze([
  // GUI / notebook stacks are not used by the desktop NDJSON worker.
  'PyQt5',
  'PyQt6',
  'PySide2',
  'PySide6',
  'tkinter',
  'IPython',
  'ipykernel',
  'jupyter',
  'notebook',
  'nbconvert',
  'nbformat',

  // Plotting and data science helpers pulled in by optional Ultralytics paths.
  'matplotlib',
  'pandas',
  'seaborn',
  'skimage',
  'sklearn',

  // Training, experiment tracking, documentation, and test-only integrations.
  'pytest',
  'sphinx',
  'tensorboard',
  'tensorboardX',
  'wandb',
  'clearml',
  'comet_ml',
  'mlflow',
  'ray',
  'dvc',

  // Model export / alternative runtime backends outside Phase 0/1 inference.
  'coremltools',
  'mss',
  'ncnn',
  'onnx',
  'onnxruntime',
  'openvino',
  'paddle',
  'paddlepaddle',
  'pycocotools',
  'tensorflow',
  'tensorrt',
  'torch',
  'torchvision',
  'ultralytics'
])

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
    windowsHide: true,
    ...options
  })

  if (result.status !== 0) {
    throw new Error(`命令执行失败: ${cmd} ${args.join(' ')}`)
  }
}

function workerPythonEnv(extra = {}) {
  return {
    ...process.env,
    MPLBACKEND: 'Agg',
    ULTRALYTICS_SKIP_REQUIREMENTS_CHECKS: '1',
    ...extra
  }
}

function verifyPackagedWorker(distDir) {
  const executable = join(distDir, bundledExecutableName('desktop_worker'))
  runCommand(executable, ['--self-check'], {
    cwd: aiDir,
    env: workerPythonEnv()
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
  const result = spawnSync(cmd, args, { stdio: 'pipe', windowsHide: true })
  return result.status === 0
}

function copyFileIfExists(source, targetDir, targetName) {
  if (!existsSync(source)) {
    throw new Error(`缺少文件: ${source}`)
  }
  ensureDir(targetDir)
  cpSync(realpathSync(source), join(targetDir, targetName))
}

function copyWindowsToolRuntime(source, targetDir, targetName, runtimeDllNames = []) {
  copyFileIfExists(source, targetDir, targetName)
  if (process.platform !== 'win32') {
    return
  }

  const sourceDir = dirname(realpathSync(source))
  const normalizedSourceDir = sourceDir.replaceAll('\\', '/').toLowerCase()
  if (!normalizedSourceDir.endsWith('/library/bin')) {
    return
  }

  const manualDlls = []
  for (const dllName of runtimeDllNames) {
    const sourceDll = join(sourceDir, dllName)
    if (!existsSync(sourceDll)) {
      continue
    }
    const targetDll = join(targetDir, dllName)
    if (!existsSync(targetDll)) {
      cpSync(sourceDll, targetDll, { dereference: true })
    }
    manualDlls.push(targetDll)
  }

  copyPeDependencies([source, ...manualDlls], [sourceDir], targetDir)
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

function pruneLegacyWorkerPackages(distDir) {
  const internalDir = join(distDir, '_internal')
  if (!existsSync(internalDir)) {
    return
  }

  for (const legacyPackage of ['onnxruntime', 'torch', 'torchvision', 'ultralytics']) {
    rmSync(join(internalDir, legacyPackage), { recursive: true, force: true })
  }
}

function pythonLiteral(value) {
  return JSON.stringify(value, null, 2)
}

function writeWorkerPyInstallerSpec() {
  writeFileSync(workerPyInstallerSpecPath, `# -*- mode: python ; coding: utf-8 -*-

block_cipher = None

a = Analysis(
    [${pythonLiteral(join(aiDir, 'desktop_worker.py'))}],
    pathex=${pythonLiteral([aiDir])},
    binaries=[],
    datas=[],
    hiddenimports=[],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=${pythonLiteral(workerPyInstallerExcludes)},
    noarchive=False,
    optimize=1,
)
pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)
exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name='desktop_worker',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=True,
)
coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=False,
    upx_exclude=[],
    name='desktop_worker',
)
`, 'utf-8')
}

function resolveWhich(binary) {
  const locator = process.platform === 'win32' ? 'where' : 'which'
  const result = spawnSync(locator, [binary], { encoding: 'utf-8', windowsHide: true })
  if (result.status !== 0) {
    throw new Error(`未找到系统命令: ${binary}`)
  }
  return result.stdout.trim().split(/\r?\n/)[0]
}

function resolveCondaCommand() {
  if (requireCommand('conda')) {
    return 'conda'
  }

  if (process.platform !== 'win32') {
    return null
  }

  const userProfile = process.env.USERPROFILE
  const candidates = [
    process.env.CONDA_EXE,
    userProfile ? join(userProfile, 'anaconda3', 'Scripts', 'conda.exe') : null,
    userProfile ? join(userProfile, 'anaconda3', 'condabin', 'conda.bat') : null,
    userProfile ? join(userProfile, 'miniconda3', 'Scripts', 'conda.exe') : null,
    userProfile ? join(userProfile, 'miniconda3', 'condabin', 'conda.bat') : null
  ].filter(Boolean)

  return candidates.find(candidate => existsSync(candidate)) ?? null
}

function resolveCondaEnvPrefix(envName) {
  const conda = resolveCondaCommand()
  if (!conda) {
    return null
  }

  const result = spawnSync(conda, ['env', 'list', '--json'], { encoding: 'utf-8', windowsHide: true })
  if (result.status !== 0) {
    return null
  }

  try {
    const parsed = JSON.parse(result.stdout)
    return parsed.envs
      ?.find(envPath => envPath.split(/[\\/]/).at(-1) === envName) ?? null
  } catch {
    return null
  }
}

function resolveRuntimeTool(binary) {
  try {
    return resolveWhich(binary)
  } catch (error) {
    if (process.platform !== 'win32') {
      throw error
    }

    const condaEnvName = process.env.TAURI_WORKER_CONDA_ENV ?? 'var-env'
    const envPrefix = process.env.CONDA_PREFIX ?? resolveCondaEnvPrefix(condaEnvName)
    const binaryName = bundledExecutableName(binary)
    const candidates = [
      envPrefix ? join(envPrefix, 'Library', 'bin', binaryName) : null,
      envPrefix ? join(envPrefix, 'Scripts', binaryName) : null
    ].filter(Boolean)
    const resolved = candidates.find(candidate => existsSync(candidate))

    if (resolved) {
      return resolved
    }

    throw error
  }
}

function resolveRequirementsFile() {
  const explicitRequirementsFile = process.env.TAURI_WORKER_REQUIREMENTS
  if (explicitRequirementsFile) {
    return resolve(explicitRequirementsFile)
  }

  if (process.platform === 'darwin') {
    return join(aiDir, 'requirements-desktop-macos.txt')
  }

  if (process.platform === 'win32') {
    const cudaRequirements = join(aiDir, 'requirements-desktop-windows-cuda.txt')
    if (existsSync(cudaRequirements)) {
      return cudaRequirements
    }

    return join(aiDir, 'requirements-desktop-windows-cpu.txt')
  }

  return join(aiDir, 'requirements-cpu.txt')
}

function resolveCondaPythonPath(envName) {
  const conda = resolveCondaCommand()
  if (!conda) {
    return null
  }

  const result = spawnSync(
    conda,
    ['run', '-n', envName, 'python', '-c', 'import sys; print(sys.executable)'],
    {
      encoding: 'utf-8',
      windowsHide: true
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
      'import PyInstaller, cv2, dotenv, numpy'
    ],
    {
      stdio: 'pipe',
      windowsHide: true,
      env: workerPythonEnv()
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
      encoding: 'utf-8',
      windowsHide: true
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
  const includedExtensions = new Set(['.py', '.yaml', '.yml', '.json'])

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

function ensureBuildPython(requirementsFile = resolveRequirementsFile()) {
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
  if (process.env.TAURI_WORKER_USE_CONDA === '1' && condaPython && existsSync(condaPython)) {
    return ensurePythonDependencies({
      python: condaPython,
      requirementsFile,
      stampPath: join(pyInstallerRoot, `conda-${condaEnvName}-ready.json`),
      pythonIdentity: `conda:${condaEnvName}:${condaPython}`
    })
  }

  const { python } = resolveVenvPaths()
  const stampPath = join(workerVenvDir, '.desktop-worker-ready.json')
  const venvExists = existsSync(python)
  const pythonCommand = venvExists
    ? null
    : requireCommand('python', ['--version'])
      ? 'python'
      : condaPython && existsSync(condaPython)
        ? condaPython
        : resolvePythonCommand()
  const pythonIdentity = pythonCommand ? `venv:${pythonCommand}` : `venv:${python}`

  let stamp = null
  if (existsSync(stampPath)) {
    try {
      stamp = JSON.parse(readFileSync(stampPath, 'utf-8'))
    } catch {
      stamp = null
    }
  }

  if (pythonCommand && stamp?.pythonIdentity && stamp.pythonIdentity !== pythonIdentity) {
    rmSync(workerVenvDir, { recursive: true, force: true })
  }

  if (!existsSync(python)) {
    runCommand(pythonCommand, ['-m', 'venv', workerVenvDir])
  }

  return ensurePythonDependencies({
    python,
    requirementsFile,
    stampPath,
    pythonIdentity
  })
}

function buildWorker(buildContext = null) {
  const requirementsFile = buildContext?.requirementsFile ?? resolveRequirementsFile()
  const { python, pythonIdentity } = buildContext ?? ensureBuildPython(requirementsFile)
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
  rmSync(workerPyInstallerSpecPath, { force: true })
  ensureDir(workerResourcesDir)
  ensureDir(pyInstallerRoot)
  writeWorkerPyInstallerSpec()

  const build = spawnSync(
    python,
    [
      '-m',
      'PyInstaller',
      '--noconfirm',
      '--clean',
      '--distpath',
      join(pyInstallerRoot, 'dist'),
      '--workpath',
      join(pyInstallerRoot, 'build'),
      workerPyInstallerSpecPath
    ],
    {
      stdio: 'inherit',
      windowsHide: true,
      cwd: aiDir,
      env: workerPythonEnv()
    }
  )

  if (build.status !== 0) {
    throw new Error('PyInstaller 构建桌面 worker 失败')
  }

  const packagedWorkerDir = join(pyInstallerRoot, 'dist', 'desktop_worker')
  pruneLegacyWorkerPackages(packagedWorkerDir)
  verifyPackagedWorker(packagedWorkerDir)

  cpSync(packagedWorkerDir, cachedWorkerDir, {
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
  rmSync(legacyBinResourcesDir, { recursive: true, force: true })
  rmSync(legacyWorkerResourcesDir, { recursive: true, force: true })
  ensureDir(modelResourcesDir)
  ensureDir(toolResourcesDir)

  const ffmpegBinaryName = process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg'
  const ffprobeBinaryName = process.platform === 'win32' ? 'ffprobe.exe' : 'ffprobe'
  const ffmpegRuntimeDlls = [
    'aom.dll',
    'avcodec-60.dll',
    'avdevice-60.dll',
    'avfilter-9.dll',
    'avformat-60.dll',
    'avutil-58.dll',
    'charset.dll',
    'dav1d.dll',
    'fontconfig-1.dll',
    'freetype.dll',
    'fribidi-0.dll',
    'gmp.dll',
    'harfbuzz.dll',
    'iconv.dll',
    'intl-8.dll',
    'jpeg8.dll',
    'libbz2.dll',
    'libcrypto-3-x64.dll',
    'libexpat.dll',
    'liblzma.dll',
    'libssl-3-x64.dll',
    'libxml2.dll',
    'openh264-7.dll',
    'openjp2.dll',
    'opus.dll',
    'SvtAv1Enc.dll',
    'swresample-4.dll',
    'swscale-7.dll',
    'theora.dll',
    'theoradec.dll',
    'theoraenc.dll',
    'vorbis.dll',
    'vorbisenc.dll',
    'vorbisfile.dll',
    'zlib.dll',
    'zstd.dll'
  ]

  if (!existsSync(sourceOnnxModelPath)) {
    throw new Error(`缺少 ONNX 模型: ${sourceOnnxModelPath}。请先运行 npm run desktop:export-onnx`)
  }

  copyFileIfExists(sourceOnnxModelPath, modelResourcesDir, 'best.onnx')
  copyWindowsToolRuntime(resolveRuntimeTool('ffmpeg'), toolResourcesDir, ffmpegBinaryName, ffmpegRuntimeDlls)
  copyWindowsToolRuntime(resolveRuntimeTool('ffprobe'), toolResourcesDir, ffprobeBinaryName, ffmpegRuntimeDlls)
  normalizeBundledResource(toolResourcesDir)
}

function main() {
  const requirementsFile = resolveRequirementsFile()
  const buildContext = {
    ...ensureBuildPython(requirementsFile),
    requirementsFile
  }

  copyRuntimeResources()
  buildWorker(buildContext)
  console.log('桌面 worker 和运行资源已更新到 src-tauri/resources')
}

main()
