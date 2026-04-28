import { existsSync, readdirSync, readFileSync, cpSync } from 'node:fs'
import { basename, join } from 'node:path'

const WINDOWS_SYSTEM_DLLS = new Set([
  'advapi32.dll',
  'bcrypt.dll',
  'cfgmgr32.dll',
  'combase.dll',
  'comctl32.dll',
  'comdlg32.dll',
  'crypt32.dll',
  'd3d11.dll',
  'd3d12.dll',
  'dxgi.dll',
  'gdi32.dll',
  'imm32.dll',
  'kernel32.dll',
  'mf.dll',
  'mfplat.dll',
  'mfreadwrite.dll',
  'msvcrt.dll',
  'msvcp_win.dll',
  'ncrypt.dll',
  'ole32.dll',
  'oleaut32.dll',
  'powrprof.dll',
  'psapi.dll',
  'rpcrt4.dll',
  'secur32.dll',
  'setupapi.dll',
  'shell32.dll',
  'shlwapi.dll',
  'user32.dll',
  'userenv.dll',
  'usp10.dll',
  'uuid.dll',
  'version.dll',
  'winmm.dll',
  'wintrust.dll',
  'ws2_32.dll',
  'wsock32.dll'
])

const NVIDIA_DRIVER_DLLS = new Set([
  'nvcuda.dll',
  'nvcuvid.dll',
  'nvencodeapi64.dll'
])

export const DEFAULT_EXTERNAL_DLLS = new Set([
  ...WINDOWS_SYSTEM_DLLS,
  ...NVIDIA_DRIVER_DLLS
])

function isExternalDll(dllName, externalDlls) {
  return externalDlls.has(dllName) ||
    dllName.startsWith('api-ms-win-') ||
    dllName.startsWith('ext-ms-win-')
}

function readNullTerminatedAscii(buffer, offset) {
  let end = offset
  while (end < buffer.length && buffer[end] !== 0) {
    end += 1
  }
  return buffer.toString('ascii', offset, end)
}

function createRvaResolver(buffer, peOffset, optionalHeaderOffset, optionalHeaderSize) {
  const sectionCount = buffer.readUInt16LE(peOffset + 6)
  const sections = []
  const sectionTableOffset = optionalHeaderOffset + optionalHeaderSize

  for (let i = 0; i < sectionCount; i += 1) {
    const offset = sectionTableOffset + (i * 40)
    if (offset + 40 > buffer.length) {
      break
    }
    sections.push({
      virtualSize: buffer.readUInt32LE(offset + 8),
      virtualAddress: buffer.readUInt32LE(offset + 12),
      rawSize: buffer.readUInt32LE(offset + 16),
      rawPointer: buffer.readUInt32LE(offset + 20)
    })
  }

  return (rva) => {
    for (const section of sections) {
      const size = Math.max(section.virtualSize, section.rawSize)
      if (rva >= section.virtualAddress && rva < section.virtualAddress + size) {
        return section.rawPointer + (rva - section.virtualAddress)
      }
    }
    return null
  }
}

export function readPeImportDllNames(binaryPath) {
  const buffer = readFileSync(binaryPath)
  if (buffer.length < 0x40 || buffer.toString('ascii', 0, 2) !== 'MZ') {
    return []
  }

  const peOffset = buffer.readUInt32LE(0x3c)
  if (peOffset + 24 >= buffer.length || buffer.toString('ascii', peOffset, peOffset + 4) !== 'PE\u0000\u0000') {
    return []
  }

  const optionalHeaderSize = buffer.readUInt16LE(peOffset + 20)
  const optionalHeaderOffset = peOffset + 24
  const optionalMagic = buffer.readUInt16LE(optionalHeaderOffset)
  const dataDirectoryOffset = optionalMagic === 0x20b
    ? optionalHeaderOffset + 112
    : optionalHeaderOffset + 96
  const importTableRva = buffer.readUInt32LE(dataDirectoryOffset + 8)
  if (importTableRva === 0) {
    return []
  }

  const rvaToOffset = createRvaResolver(buffer, peOffset, optionalHeaderOffset, optionalHeaderSize)
  const importTableOffset = rvaToOffset(importTableRva)
  if (importTableOffset === null) {
    return []
  }

  const dllNames = []
  for (let offset = importTableOffset; offset + 20 <= buffer.length; offset += 20) {
    const originalFirstThunk = buffer.readUInt32LE(offset)
    const nameRva = buffer.readUInt32LE(offset + 12)
    const firstThunk = buffer.readUInt32LE(offset + 16)
    if (originalFirstThunk === 0 && nameRva === 0 && firstThunk === 0) {
      break
    }

    const nameOffset = rvaToOffset(nameRva)
    if (nameOffset !== null && nameOffset < buffer.length) {
      dllNames.push(readNullTerminatedAscii(buffer, nameOffset))
    }
  }

  return dllNames
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

  if (existsSync(root)) {
    walk(root)
  }
  return files
}

export function peFilesUnder(root) {
  return walkFiles(root).filter(path => /\.(exe|dll)$/i.test(path))
}

function indexDlls(dirs) {
  const dlls = new Map()
  for (const dir of dirs) {
    if (!dir || !existsSync(dir)) {
      continue
    }
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.isFile() && entry.name.toLowerCase().endsWith('.dll')) {
        dlls.set(entry.name.toLowerCase(), join(dir, entry.name))
      }
    }
  }
  return dlls
}

export function copyPeDependencies(entryPaths, sourceDirs, targetDir, options = {}) {
  const externalDlls = options.externalDlls ?? DEFAULT_EXTERNAL_DLLS
  const sourceDlls = indexDlls([targetDir, ...sourceDirs])
  const pending = entryPaths.flatMap(path => readPeImportDllNames(path))
  const seen = new Set()

  while (pending.length > 0) {
    const dllName = pending.pop()?.toLowerCase()
    if (!dllName || seen.has(dllName) || isExternalDll(dllName, externalDlls)) {
      continue
    }
    seen.add(dllName)

    const sourceDll = sourceDlls.get(dllName)
    if (!sourceDll) {
      continue
    }

    const targetDll = join(targetDir, basename(sourceDll))
    if (!existsSync(targetDll)) {
      cpSync(sourceDll, targetDll, { dereference: true })
      sourceDlls.set(dllName, targetDll)
    }

    pending.push(...readPeImportDllNames(targetDll))
  }
}

export function verifyPeDependencies(root, options = {}) {
  const externalDlls = options.externalDlls ?? DEFAULT_EXTERNAL_DLLS
  const peFiles = peFilesUnder(root)
  const packagedDlls = new Set(
    peFiles
      .filter(path => path.toLowerCase().endsWith('.dll'))
      .map(path => basename(path).toLowerCase())
  )
  const unresolved = []

  for (const file of peFiles) {
    for (const importName of readPeImportDllNames(file)) {
      const dllName = importName.toLowerCase()
      if (isExternalDll(dllName, externalDlls) || packagedDlls.has(dllName)) {
        continue
      }
      unresolved.push({ file, dll: importName })
    }
  }

  return unresolved
}
