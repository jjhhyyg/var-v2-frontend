import { createHash } from 'node:crypto'
import { cpSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import { verifyPeDependencies } from './pe-deps.mjs'

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
const packageLockPath = join(runtimeSourceDir, 'runtime-package-lock.json')
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
      ['a', '-tzip', '-mx=9', '-mmt=on', '-r', packagePath, '.\\*'],
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
  assertExists(join(runtimeSourceDir, 'tools', 'var-gpu-preprocessor.exe'), 'GPU preprocessor sidecar')
  assertExists(join(runtimeSourceDir, 'tools', 'var-video-analyzer.exe'), 'C++ video analyzer sidecar')
  assertExists(join(runtimeSourceDir, 'tools', 'opencv_world4140.dll'), 'OpenCV world DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'opencv_videoio_ffmpeg4140_64.dll'), 'OpenCV FFmpeg videoio plugin')
  assertExists(join(runtimeSourceDir, 'tools', 'avcodec-60.dll'), 'FFmpeg avcodec DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'avformat-60.dll'), 'FFmpeg avformat DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'avutil-58.dll'), 'FFmpeg avutil DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'swscale-7.dll'), 'FFmpeg swscale DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'fontconfig-1.dll'), 'FFmpeg fontconfig DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'libbz2.dll'), 'FFmpeg bzip2 DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'libcrypto-3-x64.dll'), 'FFmpeg OpenSSL crypto DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'libexpat.dll'), 'FFmpeg expat DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'liblzma.dll'), 'FFmpeg lzma DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'libssl-3-x64.dll'), 'FFmpeg OpenSSL SSL DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'libxml2.dll'), 'FFmpeg libxml2 DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'cudart64_12.dll'), 'CUDA runtime DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'cublas64_12.dll'), 'CUDA cuBLAS DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'cublasLt64_12.dll'), 'CUDA cuBLASLt DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'cudnn64_9.dll'), 'cuDNN DLL')
  assertExists(join(runtimeSourceDir, 'tools', 'onnxruntime.dll'), 'ONNX Runtime DLL')
  assertExists(join(modelSourceDir, 'best.onnx'), 'best.onnx')

  rmSync(stagingRoot, { recursive: true, force: true })
  mkdirSync(stagingRoot, { recursive: true })
  mkdirSync(outputDir, { recursive: true })
  rmSync(packagePath, { force: true })
  rmSync(packageLockPath, { force: true })

  cpSync(join(runtimeSourceDir, 'worker'), join(stagingRoot, 'worker'), { recursive: true, dereference: true })
  cpSync(join(runtimeSourceDir, 'tools'), join(stagingRoot, 'tools'), { recursive: true, dereference: true })
  cpSync(modelSourceDir, join(stagingRoot, 'models'), { recursive: true, dereference: true })

  const unresolved = verifyPeDependencies(stagingRoot)
  if (unresolved.length > 0) {
    const details = unresolved
      .map(item => `${relative(stagingRoot, item.file).replaceAll('\\', '/')}: ${item.dll}`)
      .join('\n')
    throw new Error(`Windows 算法包存在未随包携带的 DLL 依赖:\n${details}`)
  }

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
  const packageStats = statSync(packagePath)
  const packageSha256 = sha256(packagePath)
  writeFileSync(join(runtimeSourceDir, 'runtime-package-lock.json'), JSON.stringify({
    platform,
    runtimeBuildId,
    appVersion,
    packageName,
    size: packageStats.size,
    sha256: packageSha256,
    createdAt: new Date().toISOString()
  }, null, 2))

  console.log(`Windows CUDA Runtime 已生成: ${packagePath}`)
  console.log(`Windows CUDA Runtime SHA256: ${packageSha256}`)
  console.log(`Windows CUDA Runtime 锁定文件已生成: ${packageLockPath}`)
}

main()
