import { createHash } from 'node:crypto'
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const __dirname = dirname(fileURLToPath(import.meta.url))
const frontendDir = resolve(__dirname, '..')
const tauriConfigPath = join(frontendDir, 'src-tauri', 'tauri.conf.json')
const tauriConfig = JSON.parse(readFileSync(tauriConfigPath, 'utf-8'))
const appVersion = tauriConfig.version
const platform = 'windows-x64'
const runtimeBuildId = appVersion
const resourcesDir = join(frontendDir, 'src-tauri', 'resources')
const runtimeSourceDir = join(resourcesDir, 'runtime', platform)
const modelSourceDir = join(resourcesDir, 'models')
const outputDir = join(frontendDir, 'src-tauri', 'target', 'release', 'bundle', 'runtime')
const stagingRoot = join(frontendDir, '.windows-runtime-package')
const packageName = `VAR-Desktop-CUDA-Runtime-${platform}-${runtimeBuildId}.zip`
const packagePath = join(outputDir, packageName)
const sevenZipCandidates = [
  '7z',
  '7z.exe',
  process.env.ProgramFiles ? join(process.env.ProgramFiles, '7-Zip', '7z.exe') : null,
  process.env['ProgramFiles(x86)'] ? join(process.env['ProgramFiles(x86)'], '7-Zip', '7z.exe') : null
].filter(Boolean)

function assertExists(path, label) {
  if (!existsSync(path)) {
    throw new Error(`缺少 ${label}: ${path}`)
  }
}

function walkFiles(root) {
  const files = []

  function walk(current) {
    for (const entry of readdirSync(current, { withFileTypes: true })) {
      const fullPath = join(current, entry.name)
      if (entry.isDirectory()) {
        walk(fullPath)
        continue
      }
      files.push(fullPath)
    }
  }

  walk(root)
  return files
}

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex')
}

function commandAvailable(command, args = ['i']) {
  const result = spawnSync(command, args, {
    cwd: frontendDir,
    stdio: 'ignore',
    windowsHide: true
  })
  return result.status === 0
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? frontendDir,
    stdio: options.stdio ?? 'inherit',
    windowsHide: true
  })
  if (result.status !== 0) {
    throw new Error(`命令执行失败: ${command} ${args.join(' ')}`)
  }
}

function psSingleQuoted(value) {
  return `'${value.replaceAll('\'', '\'\'')}'`
}

function resolveSevenZipCommand() {
  return sevenZipCandidates.find((candidate) => {
    if (candidate.includes('\\') && !existsSync(candidate)) {
      return false
    }
    return commandAvailable(candidate)
  }) ?? null
}

function createRuntimeZip() {
  const sevenZip = resolveSevenZipCommand()
  if (sevenZip) {
    run(
      sevenZip,
      ['a', '-tzip', '-mx=0', '-mmt=on', '-r', packagePath, '.\\*'],
      { cwd: stagingRoot }
    )
    return
  }

  run('powershell', [
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-Command',
    `Compress-Archive -Path ${psSingleQuoted(`${stagingRoot}\\*`)} -DestinationPath ${psSingleQuoted(packagePath)} -Force`
  ])
}

function main() {
  assertExists(join(runtimeSourceDir, 'worker', 'desktop_worker', 'desktop_worker.exe'), 'Windows worker')
  assertExists(join(runtimeSourceDir, 'tools', 'ffmpeg.exe'), 'ffmpeg.exe')
  assertExists(join(runtimeSourceDir, 'tools', 'ffprobe.exe'), 'ffprobe.exe')
  assertExists(join(modelSourceDir, 'best.pt'), 'best.pt')

  rmSync(stagingRoot, { recursive: true, force: true })
  mkdirSync(stagingRoot, { recursive: true })
  mkdirSync(outputDir, { recursive: true })
  rmSync(packagePath, { force: true })

  cpSync(join(runtimeSourceDir, 'worker'), join(stagingRoot, 'worker'), { recursive: true, dereference: true })
  cpSync(join(runtimeSourceDir, 'tools'), join(stagingRoot, 'tools'), { recursive: true, dereference: true })
  cpSync(modelSourceDir, join(stagingRoot, 'models'), { recursive: true, dereference: true })

  const files = walkFiles(stagingRoot)
    .filter(path => !path.endsWith('runtime-manifest.json'))
    .map(path => ({
      path: relative(stagingRoot, path).replaceAll('\\', '/'),
      size: statSync(path).size,
      sha256: sha256(path)
    }))
    .sort((left, right) => left.path.localeCompare(right.path))

  writeFileSync(join(stagingRoot, 'runtime-manifest.json'), JSON.stringify({
    platform,
    runtimeBuildId,
    appVersion,
    createdAt: new Date().toISOString(),
    files
  }, null, 2))

  createRuntimeZip()

  console.log(`Windows CUDA Runtime 已生成: ${packagePath}`)
}

main()
