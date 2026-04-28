import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'
import { copyPeDependencies, peFilesUnder, verifyPeDependencies } from './pe-deps.mjs'

const __dirname = dirname(fileURLToPath(import.meta.url))
const frontendDir = resolve(__dirname, '..')
const repoRoot = resolve(frontendDir, '..')
const sourceDir = join(repoRoot, 'gpu-analyzer')
const sourceOnnxModelPath = join(repoRoot, 'ai-processor', 'weights', 'best.onnx')
const gpuPreprocessSmokeInputPath = join(repoRoot, 'golden_samples', 'sample_1.mp4')
const buildDir = join(sourceDir, 'build', 'windows-x64-release')
const runtimeToolsDir = join(frontendDir, 'src-tauri', 'resources', 'runtime', 'windows-x64', 'tools')
const depsDir = join(sourceDir, '.deps')
const onnxRuntimeVersion = '1.25.0'
const onnxRuntimeRoot = process.env.VAR_ONNXRUNTIME_ROOT ?? join(depsDir, `onnxruntime-gpu-windows-${onnxRuntimeVersion}`)

const opencvRoot = process.env.VAR_OPENCV_ROOT ?? 'F:\\Project\\opencv-cpp-gpu\\install-cuda-universal-nvcodec'
const cudaRoot = process.env.VAR_CUDA_ROOT ?? 'F:\\CUDA\\v12.9'
const videoCodecSdkRoot = process.env.VAR_VIDEO_CODEC_SDK_ROOT ?? 'C:\\Video_Codec_SDK_13.0.37'
const cmakeGenerator = process.env.VAR_CMAKE_GENERATOR ?? 'Visual Studio 17 2022'

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd ?? frontendDir,
    stdio: 'inherit',
    windowsHide: true
  })
  if (result.status !== 0) {
    throw new Error(`命令执行失败: ${command} ${args.join(' ')}`)
  }
}

function assertExists(path, label) {
  if (!existsSync(path)) {
    throw new Error(`缺少 ${label}: ${path}`)
  }
}

function copyIfExists(source, targetDir) {
  if (existsSync(source)) {
    cpSync(source, join(targetDir, source.split(/[\\/]/).at(-1)), { force: true })
  }
}

function copyMatchingFiles(sourceDir, targetDir, predicate) {
  if (!existsSync(sourceDir)) {
    return
  }
  for (const entry of readdirSync(sourceDir, { withFileTypes: true })) {
    if (entry.isFile() && predicate(entry.name)) {
      cpSync(join(sourceDir, entry.name), join(targetDir, entry.name), { force: true })
    }
  }
}

function vcRuntimeDirs() {
  const dirs = []
  if (process.env.VCToolsRedistDir) {
    dirs.push(join(process.env.VCToolsRedistDir, 'x64', 'Microsoft.VC143.CRT'))
  }

  const candidates = [
    'C:\\Program Files\\Microsoft Visual Studio\\2022\\BuildTools\\VC\\Redist\\MSVC',
    'C:\\Program Files\\Microsoft Visual Studio\\2022\\Community\\VC\\Redist\\MSVC',
    'C:\\Program Files\\Microsoft Visual Studio\\2022\\Professional\\VC\\Redist\\MSVC',
    'C:\\Program Files\\Microsoft Visual Studio\\2022\\Enterprise\\VC\\Redist\\MSVC'
  ]
  for (const candidate of candidates) {
    if (!existsSync(candidate)) {
      continue
    }
    for (const entry of readdirSync(candidate, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        dirs.push(join(candidate, entry.name, 'x64', 'Microsoft.VC143.CRT'))
      }
    }
  }

  if (process.env.SystemRoot) {
    dirs.push(join(process.env.SystemRoot, 'System32'))
  }

  return dirs.filter(existsSync)
}

function ensureOnnxRuntimePackage() {
  const header = join(onnxRuntimeRoot, 'buildTransitive', 'native', 'include', 'onnxruntime_cxx_api.h')
  const lib = join(onnxRuntimeRoot, 'runtimes', 'win-x64', 'native', 'onnxruntime.lib')
  if (existsSync(header) && existsSync(lib)) {
    return
  }

  mkdirSync(depsDir, { recursive: true })
  const packagePath = join(depsDir, `microsoft.ml.onnxruntime.gpu.windows.${onnxRuntimeVersion}.nupkg`)
  const url = `https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.Gpu.Windows/${onnxRuntimeVersion}`
  const ps = [
    '$ErrorActionPreference = "Stop"',
    `$pkg = '${packagePath.replaceAll('\'', '\'\'')}'`,
    `$out = '${onnxRuntimeRoot.replaceAll('\'', '\'\'')}'`,
    `Invoke-WebRequest -Uri '${url}' -OutFile $pkg`,
    'if (Test-Path $out) { Remove-Item -LiteralPath $out -Recurse -Force }',
    'Expand-Archive -LiteralPath $pkg -DestinationPath $out -Force'
  ].join('; ')
  run('powershell', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', ps])
}

function main() {
  assertExists(join(opencvRoot, 'x64', 'vc17', 'lib', 'OpenCVConfig.cmake'), 'OpenCVConfig.cmake')
  assertExists(join(cudaRoot, 'bin', 'cudart64_12.dll'), 'CUDA runtime')
  assertExists(join(videoCodecSdkRoot, 'Interface', 'nvEncodeAPI.h'), 'Video Codec SDK headers')
  ensureOnnxRuntimePackage()
  assertExists(join(onnxRuntimeRoot, 'buildTransitive', 'native', 'include', 'onnxruntime_cxx_api.h'), 'ONNX Runtime C++ headers')
  assertExists(join(onnxRuntimeRoot, 'runtimes', 'win-x64', 'native', 'onnxruntime.lib'), 'ONNX Runtime import library')

  rmSync(buildDir, { recursive: true, force: true })
  rmSync(runtimeToolsDir, { recursive: true, force: true })
  mkdirSync(buildDir, { recursive: true })
  mkdirSync(runtimeToolsDir, { recursive: true })

  run('cmake', [
    '-S', sourceDir,
    '-B', buildDir,
    '-G', cmakeGenerator,
    '-A', 'x64',
    `-DOpenCV_DIR=${join(opencvRoot, 'x64', 'vc17', 'lib')}`,
    `-DCUDAToolkit_ROOT=${cudaRoot}`,
    `-DVIDEO_CODEC_SDK_ROOT=${videoCodecSdkRoot}`,
    `-DONNXRUNTIME_ROOT=${onnxRuntimeRoot}`
  ])

  run('cmake', ['--build', buildDir, '--config', 'Release'])

  const exePath = join(buildDir, 'Release', 'var-gpu-preprocessor.exe')
  assertExists(exePath, 'var-gpu-preprocessor.exe')
  cpSync(exePath, join(runtimeToolsDir, 'var-gpu-preprocessor.exe'), { force: true })

  const analyzerExePath = join(buildDir, 'Release', 'var-video-analyzer.exe')
  assertExists(analyzerExePath, 'var-video-analyzer.exe')
  cpSync(analyzerExePath, join(runtimeToolsDir, 'var-video-analyzer.exe'), { force: true })

  copyIfExists(join(opencvRoot, 'x64', 'vc17', 'bin', 'opencv_world4140.dll'), runtimeToolsDir)
  copyIfExists(join(opencvRoot, 'x64', 'vc17', 'bin', 'opencv_videoio_ffmpeg4140_64.dll'), runtimeToolsDir)

  copyIfExists(join(cudaRoot, 'bin', 'cudart64_12.dll'), runtimeToolsDir)
  copyIfExists(join(cudaRoot, 'bin', 'cublas64_12.dll'), runtimeToolsDir)
  copyIfExists(join(cudaRoot, 'bin', 'cublasLt64_12.dll'), runtimeToolsDir)
  copyIfExists(join(cudaRoot, 'bin', 'cufft64_11.dll'), runtimeToolsDir)
  copyIfExists(join(cudaRoot, 'bin', 'curand64_10.dll'), runtimeToolsDir)
  copyIfExists(join(cudaRoot, 'bin', 'nvrtc64_120_0.dll'), runtimeToolsDir)
  copyIfExists(join(cudaRoot, 'bin', 'nvrtc-builtins64_129.dll'), runtimeToolsDir)
  copyMatchingFiles(join(cudaRoot, 'bin'), runtimeToolsDir, name => /^cudnn.*64_9\.dll$/i.test(name))
  copyMatchingFiles(join(cudaRoot, 'bin'), runtimeToolsDir, name => /^npp.*64_12\.dll$/i.test(name))
  copyMatchingFiles(
    join(onnxRuntimeRoot, 'runtimes', 'win-x64', 'native'),
    runtimeToolsDir,
    name => /^onnxruntime.*\.dll$/i.test(name) && !/^onnxruntime_providers_tensorrt\.dll$/i.test(name)
  )
  copyPeDependencies(peFilesUnder(runtimeToolsDir), [
    runtimeToolsDir,
    join(opencvRoot, 'x64', 'vc17', 'bin'),
    join(cudaRoot, 'bin'),
    join(onnxRuntimeRoot, 'runtimes', 'win-x64', 'native'),
    ...vcRuntimeDirs()
  ], runtimeToolsDir)

  const unresolved = verifyPeDependencies(runtimeToolsDir)
  if (unresolved.length > 0) {
    const details = unresolved
      .map(item => `${item.file}: ${item.dll}`)
      .join('\n')
    throw new Error(`GPU sidecar runtime 依赖缺失:\n${details}`)
  }

  run(join(runtimeToolsDir, 'var-gpu-preprocessor.exe'), ['--self-check'], { cwd: runtimeToolsDir })
  if (existsSync(gpuPreprocessSmokeInputPath) && process.env.VAR_SKIP_GPU_PREPROCESS_SMOKE !== '1') {
    const smokeOutputPath = join(buildDir, 'preprocess-smoke.mp4')
    rmSync(smokeOutputPath, { force: true })
    run(
      join(runtimeToolsDir, 'var-gpu-preprocessor.exe'),
      ['--input', gpuPreprocessSmokeInputPath, '--output', smokeOutputPath, '--fps', '25.0'],
      { cwd: runtimeToolsDir }
    )
    assertExists(smokeOutputPath, 'GPU preprocessor smoke output')
    rmSync(smokeOutputPath, { force: true })
  }
  if (existsSync(sourceOnnxModelPath)) {
    run(
      join(runtimeToolsDir, 'var-video-analyzer.exe'),
      ['--self-check-onnx', '--model', sourceOnnxModelPath],
      { cwd: runtimeToolsDir }
    )
  }

  console.log(`GPU sidecars 已生成: ${runtimeToolsDir}`)
}

main()
