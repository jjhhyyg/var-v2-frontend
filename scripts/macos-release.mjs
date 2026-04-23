import { existsSync, mkdirSync, readdirSync, rmSync, statSync } from 'node:fs'
import { basename, dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const __dirname = dirname(fileURLToPath(import.meta.url))
const frontendDir = resolve(__dirname, '..')
const bundleDir = resolve(frontendDir, 'src-tauri', 'target', 'release', 'bundle')
const appPath = resolve(bundleDir, 'macos', 'VAR Desktop.app')
const dmgPath = resolve(bundleDir, 'dmg', 'VAR Desktop_0.1.0_aarch64.dmg')
const tempDir = resolve(frontendDir, '.tauri-signing')
const appZipPath = resolve(tempDir, 'VAR-Desktop-macos.zip')
const appIdentifier = 'cn.edu.ustb.hyy.var-desktop'

const args = new Set(process.argv.slice(2))
const shouldBuild = args.has('--build')
const verifyOnly = args.has('--verify')
const preflightOnly = args.has('--preflight')
const allowAdhoc = args.has('--allow-adhoc') || process.env.MACOS_ALLOW_ADHOC_SIGN === '1'
const tauriBuildAppleEnv = [
  'APPLE_SIGNING_IDENTITY',
  'APPLE_API_KEY',
  'APPLE_API_ISSUER',
  'APPLE_API_KEY_PATH',
  'APPLE_ID',
  'APPLE_PASSWORD',
  'APPLE_TEAM_ID'
]

function run(command, commandArgs, options = {}) {
  const result = spawnSync(command, commandArgs, {
    encoding: 'utf-8',
    stdio: options.stdio ?? 'pipe',
    cwd: options.cwd ?? frontendDir,
    env: {
      ...process.env,
      ...options.env
    }
  })

  if (result.status !== 0 && options.check !== false) {
    const stderr = result.stderr?.trim()
    const stdout = result.stdout?.trim()
    const detail = stderr || stdout
    throw new Error(`命令执行失败: ${command} ${commandArgs.join(' ')}${detail ? `\n${detail}` : ''}`)
  }

  return result
}

function collectFiles(root) {
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

function isMachO(path) {
  if (!existsSync(path)) {
    return false
  }

  const result = run('file', ['-b', path], { check: false })
  return result.status === 0 && /Mach-O/.test(result.stdout)
}

function fileDepth(path) {
  return path.split('/').length
}

function listDeveloperIdIdentities() {
  const result = run('security', ['find-identity', '-v', '-p', 'codesigning'], { check: false })
  if (result.status !== 0) {
    return []
  }

  return result.stdout
    .split(/\r?\n/)
    .map(line => line.trim())
    .filter(line => line.includes('Developer ID Application:'))
    .map(line => {
      const match = line.match(/"(.+?)"/)
      return match?.[1] ?? ''
    })
    .filter(Boolean)
}

function resolveSigningIdentity() {
  const explicitIdentity = process.env.APPLE_SIGNING_IDENTITY?.trim()
  if (explicitIdentity) {
    return explicitIdentity
  }

  const identities = listDeveloperIdIdentities()
  if (identities.length > 0) {
    return identities[0]
  }

  if (allowAdhoc) {
    return '-'
  }

  throw new Error(
    [
      '未找到可用的 Developer ID Application 证书。',
      '解决方式：',
      '1. 在钥匙串中安装 Developer ID Application 证书',
      '2. 或设置 APPLE_SIGNING_IDENTITY 指向该证书名称',
      '3. 若仅做本机验链，可加 --allow-adhoc 使用 ad-hoc 签名'
    ].join('\n')
  )
}

function resolveNotarizationMode() {
  const apiKey = process.env.APPLE_API_KEY?.trim()
  const apiIssuer = process.env.APPLE_API_ISSUER?.trim()
  const apiKeyPath = process.env.APPLE_API_KEY_PATH?.trim()

  if (apiKey && apiIssuer) {
    if (!apiKeyPath) {
      throw new Error('检测到 APPLE_API_KEY / APPLE_API_ISSUER，但缺少 APPLE_API_KEY_PATH')
    }

    return {
      mode: 'apiKey',
      args: ['--key', apiKeyPath, '--key-id', apiKey, '--issuer', apiIssuer]
    }
  }

  const appleId = process.env.APPLE_ID?.trim()
  const applePassword = process.env.APPLE_PASSWORD?.trim()
  const appleTeamId = process.env.APPLE_TEAM_ID?.trim()

  if (appleId || applePassword || appleTeamId) {
    if (!(appleId && applePassword && appleTeamId)) {
      throw new Error('检测到 APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID 中的部分变量，但缺少完整三元组')
    }

    return {
      mode: 'appleId',
      args: ['--apple-id', appleId, '--password', applePassword, '--team-id', appleTeamId]
    }
  }

  return null
}

function resolveSigningContext({ requireNotarization = true } = {}) {
  run('xcrun', ['--find', 'notarytool'])
  const identity = resolveSigningIdentity()
  const notarization = resolveNotarizationMode()

  console.log(`签名身份: ${identity}`)
  if (identity === '-') {
    console.log('notarization: 跳过（ad-hoc 本机验链）')
    return { identity, notarization: null }
  }

  if (!notarization && requireNotarization) {
    throw new Error(
      [
        '已找到签名证书，但未找到 notarization 凭据。',
        '请设置以下任一方案：',
        '1. APPLE_API_KEY / APPLE_API_ISSUER / APPLE_API_KEY_PATH',
        '2. APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID'
      ].join('\n')
    )
  }

  if (notarization) {
    console.log(`notarization: ${notarization.mode}`)
  } else {
    console.log('notarization: 未配置（当前模式不会提交 notarization）')
  }

  return { identity, notarization }
}

function buildDesktopApp() {
  console.log('构建阶段将临时清空 Apple 签名与 notarization 环境变量，避免 tauri build 抢先提交未重签的 app。')
  const commandArgs = [
    ...tauriBuildAppleEnv.flatMap(name => ['-u', name]),
    'npm',
    'run',
    'desktop:build'
  ]
  run('env', commandArgs, { stdio: 'inherit' })
}

function signingArgs(identity, { runtime = false, identifier = null } = {}) {
  const args = ['--force', '--sign', identity]

  if (identifier) {
    args.push('--identifier', identifier)
  }

  if (identity !== '-') {
    args.push('--timestamp')
    if (runtime) {
      args.push('--options', 'runtime')
    }
  }

  return args
}

function signFile(identity, path, options = {}) {
  run('codesign', [...signingArgs(identity, options), path], { stdio: 'inherit' })
}

function isExecutableMode(path) {
  return (statSync(path).mode & 0o111) !== 0
}

function collectCodeTargets(root) {
  return collectFiles(root)
    .filter(path => {
      const name = basename(path)
      const relativePath = relative(root, path)
      const segments = relativePath.split('/')
      const isBinaryLocation = segments.includes('bin') || segments.includes('tools')
      return name.endsWith('.dylib') || name.endsWith('.so') || isExecutableMode(path) || isBinaryLocation || name === 'desktop_worker'
    })
    .filter(isMachO)
    .sort((left, right) => fileDepth(right) - fileDepth(left))
}

function shouldEnableRuntime(path) {
  const name = basename(path)
  return !name.endsWith('.dylib') && !name.endsWith('.so')
}

function formatPath(path) {
  return relative(frontendDir, path) || path
}

function readSignatureDetails(path) {
  const result = run('codesign', ['-dv', '--verbose=4', path], { check: false })
  const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`.trim()

  return {
    ok: result.status === 0,
    adhoc: /Signature=adhoc/.test(output),
    hasDeveloperId: /Authority=Developer ID Application:/.test(output),
    hasTimestamp: /Timestamp=/.test(output),
    hasRuntime: /flags=0x[0-9a-f]+\(runtime\)/i.test(output) || /Runtime Version=/.test(output),
    output
  }
}

function assertTrustedSignature(identity, path) {
  if (identity === '-') {
    return
  }

  const details = readSignatureDetails(path)
  const requireRuntime = shouldEnableRuntime(path)

  if (!details.ok) {
    throw new Error(`无法读取签名信息: ${formatPath(path)}\n${details.output}`)
  }

  if (details.adhoc) {
    throw new Error(`嵌套二进制仍然是 ad-hoc 签名: ${formatPath(path)}`)
  }

  if (!details.hasDeveloperId) {
    throw new Error(`嵌套二进制未使用 Developer ID 证书签名: ${formatPath(path)}`)
  }

  if (!details.hasTimestamp) {
    throw new Error(`嵌套二进制缺少 secure timestamp: ${formatPath(path)}`)
  }

  if (requireRuntime && !details.hasRuntime) {
    throw new Error(`嵌套可执行文件未启用 hardened runtime: ${formatPath(path)}`)
  }
}

function signBundle(identity) {
  if (!existsSync(appPath)) {
    throw new Error(`缺少 app 产物: ${appPath}`)
  }

  const runtimeRoot = join(appPath, 'Contents', 'Resources', 'resources', 'runtime')
  const nestedCode = collectCodeTargets(runtimeRoot)

  console.log(`准备签名 runtime 二进制，共 ${nestedCode.length} 个`)

  for (const [index, filePath] of nestedCode.entries()) {
    if (index === 0 || (index + 1) % 200 === 0 || index === nestedCode.length - 1) {
      console.log(`[sign ${index + 1}/${nestedCode.length}] ${formatPath(filePath)}`)
    }

    signFile(identity, filePath, { runtime: shouldEnableRuntime(filePath) })
  }

  const mainExecutable = join(appPath, 'Contents', 'MacOS', 'var-desktop')
  signFile(identity, mainExecutable, {
    runtime: true,
    identifier: appIdentifier
  })

  run(
    'codesign',
    [
      ...signingArgs(identity, {
        runtime: true,
        identifier: appIdentifier
      }),
      '--deep',
      appPath
    ],
    { stdio: 'inherit' }
  )

  if (existsSync(dmgPath)) {
    signFile(identity, dmgPath, { identifier: `${appIdentifier}.dmg` })
  }
}

function verifyGatekeeper(expectNotarized) {
  const result = run('spctl', ['-a', '-vv', appPath], { check: false })
  const output = `${result.stdout ?? ''}\n${result.stderr ?? ''}`.trim()

  if (result.status === 0) {
    if (output) {
      console.log(output)
    }
    return
  }

  if (!expectNotarized && /Unnotarized Developer ID/.test(output)) {
    console.log('spctl 提示 Unnotarized Developer ID，说明当前包已完成 Developer ID 签名，但尚未 notarize；在提交 notarization 前这是预期现象。')
    console.log(output)
    return
  }

  throw new Error(`Gatekeeper 校验失败\n${output}`)
}

function verifyBundle({ expectTrusted, expectNotarized }) {
  if (!existsSync(appPath)) {
    throw new Error(`缺少 app 产物: ${appPath}`)
  }

  const runtimeRoot = join(appPath, 'Contents', 'Resources', 'resources', 'runtime')
  const nestedCode = collectCodeTargets(runtimeRoot)

  if (expectTrusted) {
    console.log(`校验 runtime 二进制签名，共 ${nestedCode.length} 个`)

    for (const [index, filePath] of nestedCode.entries()) {
      if (index === 0 || (index + 1) % 200 === 0 || index === nestedCode.length - 1) {
        console.log(`[verify ${index + 1}/${nestedCode.length}] ${formatPath(filePath)}`)
      }

      assertTrustedSignature('developer-id', filePath)
    }
  }

  run('codesign', ['--verify', '--deep', '--strict', '--verbose=2', appPath], { stdio: 'inherit' })
  run('codesign', ['-dv', appPath], { stdio: 'inherit' })

  if (expectTrusted) {
    verifyGatekeeper(expectNotarized)
  } else {
    console.log('ad-hoc 模式下 spctl 被拒绝属于预期现象，重点看 codesign --verify 是否通过。')
    run('spctl', ['-a', '-vv', appPath], { stdio: 'inherit', check: false })
  }
}

function notarizeBundle(identity, notarization) {
  if (identity === '-' || !notarization) {
    return
  }

  rmSync(tempDir, { recursive: true, force: true })
  mkdirSync(tempDir, { recursive: true })
  run('ditto', ['-c', '-k', '--keepParent', appPath, appZipPath], { stdio: 'inherit' })
  console.log('开始提交 app 到 Apple notarization。这个阶段可能会静默等待数分钟，终端没有新输出不代表卡死。')
  run('xcrun', ['notarytool', 'submit', appZipPath, '--wait', ...notarization.args], { stdio: 'inherit' })
  run('xcrun', ['stapler', 'staple', appPath], { stdio: 'inherit' })
  run('xcrun', ['stapler', 'validate', appPath], { stdio: 'inherit' })

  if (existsSync(dmgPath)) {
    console.log('app notarization 完成，开始提交 dmg 到 Apple notarization。这个阶段也可能静默等待数分钟。')
    run('xcrun', ['notarytool', 'submit', dmgPath, '--wait', ...notarization.args], { stdio: 'inherit' })
    run('xcrun', ['stapler', 'staple', dmgPath], { stdio: 'inherit' })
    run('xcrun', ['stapler', 'validate', dmgPath], { stdio: 'inherit' })
  }
}

function main() {
  const { identity, notarization } = resolveSigningContext({
    requireNotarization: preflightOnly || (!verifyOnly && !allowAdhoc)
  })

  if (preflightOnly) {
    return
  }

  if (shouldBuild) {
    buildDesktopApp()
  }

  signBundle(identity)
  verifyBundle({
    expectTrusted: identity !== '-',
    expectNotarized: false
  })

  if (!verifyOnly && notarization) {
    notarizeBundle(identity, notarization)
    verifyBundle({
      expectTrusted: identity !== '-',
      expectNotarized: true
    })
  }
}

try {
  main()
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
}
