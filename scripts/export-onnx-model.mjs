import { existsSync, mkdirSync, statSync, writeFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const __dirname = dirname(fileURLToPath(import.meta.url))
const frontendDir = resolve(__dirname, '..')
const rootDir = resolve(frontendDir, '..')
const aiDir = join(rootDir, 'ai-processor')
const pyInstallerRoot = join(frontendDir, '.tauri-worker-build')
const onnxExportStampPath = join(pyInstallerRoot, 'onnx-export-stamp.json')
const sourcePtModelPath = join(aiDir, 'weights', 'best.pt')
const sourceOnnxModelPath = join(aiDir, 'weights', 'best.onnx')
const onnxExportConfig = Object.freeze({
  format: 'onnx',
  imgsz: (process.env.TAURI_WORKER_ONNX_IMGSZ ?? '640,1024')
    .split(',')
    .map(value => Number(value.trim())),
  opset: Number(process.env.TAURI_WORKER_ONNX_OPSET ?? 17),
  dynamic: false,
  simplify: false,
  half: false,
  nms: false
})

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

function boolArg(value) {
  return value ? '1' : '0'
}

function validateOnnxExportConfig() {
  const imageSizes = Array.isArray(onnxExportConfig.imgsz)
    ? onnxExportConfig.imgsz
    : [onnxExportConfig.imgsz]

  if (
    imageSizes.length === 0
    || imageSizes.length > 2
    || imageSizes.some(value => !Number.isInteger(value) || value <= 0)
  ) {
    throw new Error(`非法 ONNX imgsz: ${onnxExportConfig.imgsz}`)
  }

  if (!Number.isInteger(onnxExportConfig.opset) || onnxExportConfig.opset <= 0) {
    throw new Error(`非法 ONNX opset: ${onnxExportConfig.opset}`)
  }
}

function resolveCondaCommand() {
  const result = spawnSync('conda', ['--version'], { stdio: 'ignore', windowsHide: true })
  if (result.status === 0) {
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

function resolveExportPython() {
  const explicitPython = process.env.TAURI_ONNX_EXPORT_PYTHON ?? process.env.TAURI_WORKER_PYTHON
  if (explicitPython && existsSync(explicitPython)) {
    return explicitPython
  }

  const conda = resolveCondaCommand()
  const condaEnvName = process.env.TAURI_WORKER_CONDA_ENV ?? 'var-env'
  if (conda) {
    const result = spawnSync(
      conda,
      ['run', '-n', condaEnvName, 'python', '-c', 'import sys; print(sys.executable)'],
      { encoding: 'utf-8', windowsHide: true }
    )
    if (result.status === 0) {
      const python = result.stdout
        .split(/\r?\n/)
        .map(line => line.trim())
        .filter(Boolean)
        .at(-1)
      if (python && existsSync(python)) {
        return python
      }
    }
  }

  return process.platform === 'win32' ? 'python' : 'python3'
}

function exportOnnxModel(python) {
  const exportScript = `
import shutil
import sys
import json
from pathlib import Path

try:
    import torch
    from ultralytics import YOLO
except Exception as exc:
    raise SystemExit(f"导出 ONNX 需要 torch + ultralytics 构建环境: {exc}") from exc

if not torch.cuda.is_available():
    raise SystemExit("导出 ONNX 需要可用 NVIDIA GPU / CUDA；未检测到 torch CUDA")

source = Path(sys.argv[1]).resolve()
target = Path(sys.argv[2]).resolve()
imgsz = json.loads(sys.argv[3])
opset = int(sys.argv[4])
dynamic = sys.argv[5] == "1"
simplify = sys.argv[6] == "1"
half = sys.argv[7] == "1"
nms = sys.argv[8] == "1"

model = YOLO(str(source))
exported = model.export(
    format="onnx",
    imgsz=imgsz,
    opset=opset,
    dynamic=dynamic,
    simplify=simplify,
    half=half,
    nms=nms,
    device=0,
)
exported_path = Path(exported).resolve()
target.parent.mkdir(parents=True, exist_ok=True)
if exported_path != target:
    shutil.copy2(exported_path, target)
print(f"ONNX export ready: {target}")
`

  runCommand(
    python,
    [
      '-c',
      exportScript,
      sourcePtModelPath,
      sourceOnnxModelPath,
      JSON.stringify(onnxExportConfig.imgsz),
      String(onnxExportConfig.opset),
      boolArg(onnxExportConfig.dynamic),
      boolArg(onnxExportConfig.simplify),
      boolArg(onnxExportConfig.half),
      boolArg(onnxExportConfig.nms)
    ],
    {
      cwd: aiDir,
      env: {
        ...process.env,
        ULTRALYTICS_SKIP_REQUIREMENTS_CHECKS: '1'
      }
    }
  )
}

function writeStamp(python) {
  mkdirSync(pyInstallerRoot, { recursive: true })
  const sourceStats = statSync(sourcePtModelPath)
  const targetStats = statSync(sourceOnnxModelPath)
  writeFileSync(onnxExportStampPath, JSON.stringify({
    sourceModel: sourcePtModelPath,
    sourceMtime: Math.floor(sourceStats.mtimeMs),
    sourceSize: sourceStats.size,
    targetModel: sourceOnnxModelPath,
    targetMtime: Math.floor(targetStats.mtimeMs),
    targetSize: targetStats.size,
    python,
    exportConfig: onnxExportConfig
  }, null, 2))
}

function main() {
  validateOnnxExportConfig()
  if (!existsSync(sourcePtModelPath)) {
    throw new Error(`缺少 ONNX 导出源模型: ${sourcePtModelPath}`)
  }

  const python = resolveExportPython()
  exportOnnxModel(python)
  writeStamp(python)

  console.log(`ONNX 模型已导出: ${sourceOnnxModelPath}`)
}

main()
