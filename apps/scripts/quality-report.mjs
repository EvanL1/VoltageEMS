import { mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs'
import { dirname, extname, resolve } from 'node:path'
import { spawnSync } from 'node:child_process'

const cwd = process.cwd()
const args = process.argv.slice(2)

const findArgValue = (flag) => {
  const index = args.indexOf(flag)
  if (index === -1) return null
  return args[index + 1] ?? null
}

const outArg = findArgValue('--out')
const reportPath = resolve(cwd, outArg || 'reports/quality/quality-report.md')
const coverageSummaryPath = resolve(cwd, 'coverage/coverage-summary.json')
const distPath = resolve(cwd, 'dist')

const coverageThresholds = {
  lines: 70,
  statements: 70,
  functions: 70,
  branches: 60,
}

const bundleThresholds = {
  jsBytes: 500 * 1024,
  cssBytes: 250 * 1024,
  distBytes: 15 * 1024 * 1024,
}

const run = (command, commandArgs) => {
  const result = spawnSync(command, commandArgs, {
    cwd,
    stdio: 'pipe',
    shell: process.platform === 'win32',
    encoding: 'utf8',
  })

  if (result.stdout) process.stdout.write(result.stdout)
  if (result.stderr) process.stderr.write(result.stderr)

  return {
    ok: result.status === 0,
    status: result.status ?? 1,
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? '',
  }
}

const formatPercent = (value) => `${Number(value ?? 0).toFixed(2)}%`
const formatBytes = (value) => `${(Number(value ?? 0) / 1024).toFixed(2)} KB`

const summarizeCoverage = () => {
  const summary = JSON.parse(readFileSync(coverageSummaryPath, 'utf8'))
  const total = summary.total || {}

  return {
    lines: total.lines?.pct ?? 0,
    statements: total.statements?.pct ?? 0,
    functions: total.functions?.pct ?? 0,
    branches: total.branches?.pct ?? 0,
  }
}

const walkFiles = (dir) => {
  const entries = readdirSync(dir, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const fullPath = resolve(dir, entry.name)
    if (entry.isDirectory()) {
      files.push(...walkFiles(fullPath))
    } else {
      files.push(fullPath)
    }
  }
  return files
}

const summarizeDist = () => {
  const files = walkFiles(distPath)
  const summary = {
    totalBytes: 0,
    maxJsBytes: 0,
    maxCssBytes: 0,
  }

  for (const file of files) {
    const size = statSync(file).size
    summary.totalBytes += size
    const extension = extname(file).toLowerCase()
    if (extension === '.js') {
      summary.maxJsBytes = Math.max(summary.maxJsBytes, size)
    }
    if (extension === '.css') {
      summary.maxCssBytes = Math.max(summary.maxCssBytes, size)
    }
  }

  return summary
}

const summarizeAudit = (result) => {
  const output = `${result.stdout}\n${result.stderr}`.trim()
  const severityCounts = {
    critical: 0,
    high: 0,
    moderate: 0,
    low: 0,
  }

  const matchers = {
    critical: /critical[:\s]+(\d+)/i,
    high: /\bhigh[:\s]+(\d+)/i,
    moderate: /moderate[:\s]+(\d+)/i,
    low: /\blow[:\s]+(\d+)/i,
  }

  for (const [key, pattern] of Object.entries(matchers)) {
    const match = output.match(pattern)
    if (match) {
      severityCounts[key] = Number(match[1] || 0)
    }
  }

  const unavailable =
    /NOT_IMPLEMENTED/i.test(output) ||
    /security\/audits\/quick/i.test(output) ||
    /audit endpoint returned an error/i.test(output) ||
    /not implemented/i.test(output)

  return {
    ...severityCounts,
    unavailable,
    output,
  }
}

const checkCoverageThresholds = (coverage) =>
  Object.entries(coverageThresholds).map(([key, threshold]) => ({
    key,
    threshold,
    actual: coverage[key],
    pass: Number(coverage[key] ?? 0) >= threshold,
  }))

const checkBundleThresholds = (distSummary) => [
  {
    key: 'maxJsBytes',
    label: 'Max JS bundle',
    actual: distSummary.maxJsBytes,
    threshold: bundleThresholds.jsBytes,
    pass: distSummary.maxJsBytes <= bundleThresholds.jsBytes,
  },
  {
    key: 'maxCssBytes',
    label: 'Max CSS bundle',
    actual: distSummary.maxCssBytes,
    threshold: bundleThresholds.cssBytes,
    pass: distSummary.maxCssBytes <= bundleThresholds.cssBytes,
  },
  {
    key: 'totalBytes',
    label: 'Total dist size',
    actual: distSummary.totalBytes,
    threshold: bundleThresholds.distBytes,
    pass: distSummary.totalBytes <= bundleThresholds.distBytes,
  },
]

const typeCheck = run('pnpm', ['type-check:only'])
const lint = run('pnpm', ['lint:check'])
const format = run('pnpm', ['format:check'])
const tests = run('pnpm', ['test:coverage'])
const build = run('pnpm', ['build:check'])
const audit = run('pnpm', ['audit:check'])

let coverage = null
if (tests.ok) {
  coverage = summarizeCoverage()
}

let distSummary = null
if (build.ok) {
  distSummary = summarizeDist()
}

const coverageChecks = coverage ? checkCoverageThresholds(coverage) : []
const bundleChecks = distSummary ? checkBundleThresholds(distSummary) : []
const auditSummary = summarizeAudit(audit)
const auditPass =
  auditSummary.unavailable || (auditSummary.critical === 0 && auditSummary.high === 0)
const auditStatus = auditSummary.unavailable
  ? 'UNAVAILABLE'
  : audit.ok && auditPass
    ? 'PASS'
    : `FAIL (exit ${audit.status})`

const markdown = [
  '# Quality Report',
  '',
  `- Generated at: ${new Date().toISOString()}`,
  `- Type check: ${typeCheck.ok ? 'PASS' : `FAIL (exit ${typeCheck.status})`}`,
  `- ESLint: ${lint.ok ? 'PASS' : `FAIL (exit ${lint.status})`}`,
  `- Prettier: ${format.ok ? 'PASS' : `FAIL (exit ${format.status})`}`,
  `- Test coverage: ${tests.ok ? 'PASS' : `FAIL (exit ${tests.status})`}`,
  `- Build: ${build.ok ? 'PASS' : `FAIL (exit ${build.status})`}`,
  `- Dependency audit: ${auditStatus}`,
  '',
  '## Coverage Summary',
  '',
  coverage ? `- Lines: ${formatPercent(coverage.lines)}` : '- Lines: unavailable',
  coverage ? `- Statements: ${formatPercent(coverage.statements)}` : '- Statements: unavailable',
  coverage ? `- Functions: ${formatPercent(coverage.functions)}` : '- Functions: unavailable',
  coverage ? `- Branches: ${formatPercent(coverage.branches)}` : '- Branches: unavailable',
  '',
  '## Coverage Gates',
  '',
  ...(coverageChecks.length
    ? coverageChecks.map(
        (item) =>
          `- ${item.key}: ${formatPercent(item.actual)} / threshold ${formatPercent(item.threshold)} / ${item.pass ? 'PASS' : 'FAIL'}`,
      )
    : ['- Coverage gates: unavailable']),
  '',
  '## Build Size',
  '',
  ...(bundleChecks.length
    ? bundleChecks.map(
        (item) =>
          `- ${item.label}: ${formatBytes(item.actual)} / threshold ${formatBytes(item.threshold)} / ${item.pass ? 'PASS' : 'FAIL'}`,
      )
    : ['- Build size: unavailable']),
  '',
  '## Dependency Audit',
  '',
  `- Critical: ${auditSummary.critical}`,
  `- High: ${auditSummary.high}`,
  `- Moderate: ${auditSummary.moderate}`,
  `- Low: ${auditSummary.low}`,
  `- Policy: critical/high fail, moderate/low warn`,
  auditSummary.unavailable
    ? '- Status: audit endpoint unavailable in current registry environment'
    : '- Status: audit completed',
  '',
  '## Output Files',
  '',
  '- Coverage HTML: `coverage/index.html`',
  '- Coverage LCOV: `coverage/lcov.info`',
  '- Coverage JSON summary: `coverage/coverage-summary.json`',
  '',
].join('\n')

mkdirSync(dirname(reportPath), { recursive: true })
writeFileSync(reportPath, markdown, 'utf8')

console.log(`Quality report written to ${reportPath}`)

if (
  !typeCheck.ok ||
  !lint.ok ||
  !format.ok ||
  !tests.ok ||
  !build.ok ||
  !auditPass ||
  coverageChecks.some((item) => !item.pass) ||
  bundleChecks.some((item) => !item.pass)
) {
  process.exit(1)
}
