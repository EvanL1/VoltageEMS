生成时间：2026-04-15T07:22:55.296Z

总体结果：WARN 警告

通过：7

警告：1

失败：0

结果解读

- 退出码 表示命令本身是否执行成功：0 为成功，非 0 为失败。
- 判定依据 表示这个检查为什么被判定为通过、警告或失败。
- 规则/阈值 表示这个检查使用的质量判断标准。

失败项摘要

- 无

警告项摘要

- 构建产物体积：构建已完成，但产物体积超出建议阈值

检查总览

| 检查项 | 结果 | 退出码 | 判定依据 | 规则/阈值 |
| --- | --- | --- | --- | --- |
| 类型检查 | PASS 通过 | 0 | 命令执行成功 | 执行 vue-tsc，命令失败则直接判定为失败。 |
| 代码规范检查 | PASS 通过 | 0 | 命令执行成功 | 执行 ESLint，只要存在 error 或命令非 0 退出即判定为失败。 |
| 格式检查 | PASS 通过 | 0 | 命令执行成功 | 执行 Prettier --check，格式不符合即判定为失败。 |
| 单元测试 | PASS 通过 | 0 | 命令执行成功 | 测试命令非 0 退出则判定为失败。 |
| 测试覆盖率 | PASS 通过 | 0 | 命令执行成功且覆盖率达到阈值 | 命令失败则直接失败；若覆盖率低于阈值（lines 70%，statements 70%，functions 70%，branches 60%），则记为警告。 |
| 构建验证 | PASS 通过 | 0 | 命令执行成功 | 构建命令非 0 退出则判定为失败。 |
| 构建产物体积 | WARN 警告 | 0 | 构建已完成，但产物体积超出建议阈值 | 脚本能分析 dist 即退出码为 0；若单个 JS 超过 800 KB、单个 CSS 超过 450 KB 或 dist 总体积超过 15 MB，则记为警告。 |
| 依赖安全扫描 | PASS 通过 | 1 | 未检测到高危、严重或中危漏洞 | 严重或高危漏洞记为失败；中危漏洞记为警告。当前 failOn=critical, high，warnOn=moderate。 |

当前阈值配置

- 覆盖率：lines 70%，statements 70%，functions 70%，branches 60%
- 产物体积：JS 800 KB，CSS 450 KB，dist 总体积 15 MB
- 安全扫描：failOn=critical, high，warnOn=moderate

类型检查

- 结果：PASS 通过
- 退出码：0
- 判定依据：命令执行成功
- 检查说明：检查 TypeScript 与 Vue 类型错误。
- 规则/阈值：执行 vue-tsc，命令失败则直接判定为失败。

命令：`pnpm type-check:only`

```text
> voltage-edge-ems@0.0.0 type-check:only D:\RushRush\VoltageEMS\apps
> vue-tsc --noEmit
```

代码规范检查

- 结果：PASS 通过
- 退出码：0
- 判定依据：命令执行成功
- 检查说明：检查 ESLint 规则、潜在缺陷与不规范写法。
- 规则/阈值：执行 ESLint，只要存在 error 或命令非 0 退出即判定为失败。

命令：`pnpm lint:check`

```text
> voltage-edge-ems@0.0.0 lint:check D:\RushRush\VoltageEMS\apps
> eslint . --max-warnings=0
```

格式检查

- 结果：PASS 通过
- 退出码：0
- 判定依据：命令执行成功
- 检查说明：检查代码是否符合 Prettier 格式要求。
- 规则/阈值：执行 Prettier --check，格式不符合即判定为失败。

命令：`pnpm format:check`

```text
> voltage-edge-ems@0.0.0 format:check D:\RushRush\VoltageEMS\apps
> prettier --check .

Checking formatting...
All matched files use Prettier code style!
```

单元测试

- 结果：PASS 通过
- 退出码：0
- 判定依据：命令执行成功
- 检查说明：执行 Vitest 单元测试。
- 规则/阈值：测试命令非 0 退出则判定为失败。

命令：`pnpm test:run`

```text
> voltage-edge-ems@0.0.0 test:run D:\RushRush\VoltageEMS\apps
> vitest run --passWithNoTests


 RUN  v3.2.4 D:/RushRush/VoltageEMS/apps

 ✓ src/utils/__tests__/common.test.ts (16 tests) 79ms
 ✓ src/utils/__tests__/csv.test.ts (5 tests) 238ms
 ✓ src/stores/__tests__/ruleChain.test.ts (7 tests) 33ms
 ✓ src/api/__tests__/devicesManagement.test.ts (7 tests) 17ms
 ✓ src/api/__tests__/alarm.test.ts (6 tests) 17ms
 ✓ src/utils/__tests__/date.test.ts (7 tests) 12ms
 ✓ src/api/__tests__/user.test.ts (6 tests) 16ms
 ✓ src/api/__tests__/channelsManagement.test.ts (6 tests) 20ms
 ✓ src/composables/__tests__/useWebSocket.test.ts (6 tests) 47ms
stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should initialize with default values
fetchTableData false { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should fetch table data successfully
fetchTableData false { page: 1, pageSize: 20, total: 0 }
fetchTableData false { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should include filters in query params when fetching data
fetchTableData false { page: 1, pageSize: 20, total: 0 }
fetchTableData true { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should handle pagination changes
fetchTableData false { page: 1, pageSize: 20, total: 0 }
fetchTableData false { page: 2, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should handle pagination changes
fetchTableData false { page: 1, pageSize: 50, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should handle delete row
fetchTableData false { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should handle delete row
fetchTableData false { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should clear filters and keyword when reloading filters
fetchTableData false { page: 1, pageSize: 20, total: 0 }
fetchTableData true { page: 1, pageSize: 20, total: 0 }

 ✓ src/composables/__tests__/useTableData.test.ts (6 tests) 61ms
 ✓ src/router/__tests__/guard.test.ts (7 tests) 4856ms
   ✓ router/guard.ts > allows whitelisted routes without auth checks  1641ms
   ✓ router/guard.ts > redirects to login when token and refresh token are missing  858ms
   ✓ router/guard.ts > clears user state when token refresh fails  1067ms
   ✓ router/guard.ts > falls back to login when guard execution throws  1209ms
 ✓ src/utils/__tests__/websocket.test.ts (6 tests) 4949ms
   ✓ utils/websocket.ts > rejects connect when user is not logged in  1901ms
   ✓ utils/websocket.ts > creates a websocket connection and flushes pending subscriptions on open  667ms
   ✓ utils/websocket.ts > reuses the same subscription id for duplicate subscriptions  1084ms
   ✓ utils/websocket.ts > sends unsubscribe messages for active subscriptions and removes unknown pending ids silently  1205ms
 ✓ src/api/__tests__/System.test.ts (5 tests) 13ms
 ✓ src/api/__tests__/rulesManagement.test.ts (3 tests) 11ms
 ✓ src/utils/__tests__/responsive.test.ts (6 tests) 11ms
 ✓ src/router/__tests__/static-routes.test.ts (4 tests) 5ms
 ✓ src/api/__tests__/userManagement.test.ts (4 tests) 9ms
 ✓ src/api/__tests__/homepage.test.ts (2 tests) 8ms
 ✓ src/stores/__tests__/global.test.ts (3 tests) 8ms
 ✓ src/components/card/__tests__/BatteryCard.test.ts (4 tests) 65ms
 ✓ src/components/card/__tests__/PVCard.test.ts (4 tests) 73ms
 ✓ src/components/card/__tests__/EnergyCard.test.ts (4 tests) 78ms
 ✓ src/api/Statistic/__tests__/overview.test.ts (1 test) 4ms
 ✓ src/utils/__tests__/directives.test.ts (3 tests) 4ms
 ✓ src/router/__tests__/index.test.ts (5 tests) 3ms
 ✓ src/router/__tests__/dynamic-routes.test.ts (8 tests) 7ms

 Test Files  26 passed (26)
      Tests  141 passed (141)
   Start at  15:21:20
   Duration  26.58s (transform 38.78s, setup 0ms, collect 83.29s, tests 10.65s, environment 119.26s, prepare 19.65s)


Browserslist: browsers data (caniuse-lite) is 10 months old. Please run:
  npx update-browserslist-db@latest
  Why you should do it regularly: https://github.com/browserslist/update-db#readme
```

测试覆盖率

- 结果：PASS 通过
- 退出码：0
- 判定依据：命令执行成功且覆盖率达到阈值
- 检查说明：执行带覆盖率的测试，并评估核心覆盖指标。
- 规则/阈值：命令失败则直接失败；若覆盖率低于阈值（lines 70%，statements 70%，functions 70%，branches 60%），则记为警告。

关键结论：

- 行覆盖率：77.58%（目标 70%）
- 语句覆盖率：77.58%（目标 70%）
- 函数覆盖率：71.11%（目标 70%）
- 分支覆盖率：79.35%（目标 60%）

命令：`pnpm test:coverage`

```text
> voltage-edge-ems@0.0.0 test:coverage D:\RushRush\VoltageEMS\apps
> vitest run --coverage --passWithNoTests


 RUN  v3.2.4 D:/RushRush/VoltageEMS/apps
      Coverage enabled with v8

 ✓ src/api/__tests__/user.test.ts (6 tests) 17ms
 ✓ src/utils/__tests__/common.test.ts (16 tests) 40ms
 ✓ src/api/__tests__/alarm.test.ts (6 tests) 20ms
 ✓ src/api/__tests__/System.test.ts (5 tests) 15ms
 ✓ src/api/__tests__/channelsManagement.test.ts (6 tests) 21ms
 ✓ src/api/__tests__/devicesManagement.test.ts (7 tests) 23ms
 ✓ src/utils/__tests__/csv.test.ts (5 tests) 230ms
 ✓ src/router/__tests__/guard.test.ts (7 tests) 1773ms
   ✓ router/guard.ts > falls back to login when guard execution throws  1447ms
 ✓ src/stores/__tests__/ruleChain.test.ts (7 tests) 42ms
 ✓ src/utils/__tests__/websocket.test.ts (6 tests) 1870ms
   ✓ utils/websocket.ts > rejects connect when user is not logged in  506ms
   ✓ utils/websocket.ts > creates a websocket connection and flushes pending subscriptions on open  1303ms
 ✓ src/composables/__tests__/useWebSocket.test.ts (6 tests) 50ms
stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should initialize with default values
fetchTableData false { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should fetch table data successfully
fetchTableData false { page: 1, pageSize: 20, total: 0 }
fetchTableData false { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should include filters in query params when fetching data
fetchTableData false { page: 1, pageSize: 20, total: 0 }
fetchTableData true { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should handle pagination changes
fetchTableData false { page: 1, pageSize: 20, total: 0 }
fetchTableData false { page: 2, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should handle pagination changes
fetchTableData false { page: 1, pageSize: 50, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should handle delete row
fetchTableData false { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should handle delete row
fetchTableData false { page: 1, pageSize: 20, total: 0 }

stdout | src/composables/__tests__/useTableData.test.ts > useTableData > should clear filters and keyword when reloading filters
fetchTableData false { page: 1, pageSize: 20, total: 0 }
fetchTableData true { page: 1, pageSize: 20, total: 0 }

 ✓ src/composables/__tests__/useTableData.test.ts (6 tests) 74ms
 ✓ src/utils/__tests__/date.test.ts (7 tests) 16ms
 ✓ src/utils/__tests__/responsive.test.ts (6 tests) 13ms
 ✓ src/api/__tests__/rulesManagement.test.ts (3 tests) 14ms
 ✓ src/api/__tests__/userManagement.test.ts (4 tests) 13ms
 ✓ src/components/card/__tests__/BatteryCard.test.ts (4 tests) 86ms
 ✓ src/components/card/__tests__/EnergyCard.test.ts (4 tests) 91ms
 ✓ src/components/card/__tests__/PVCard.test.ts (4 tests) 96ms
 ✓ src/api/__tests__/homepage.test.ts (2 tests) 9ms
 ✓ src/stores/__tests__/global.test.ts (3 tests) 11ms
 ✓ src/api/Statistic/__tests__/overview.test.ts (1 test) 9ms
 ✓ src/router/__tests__/static-routes.test.ts (4 tests) 10ms
 ✓ src/router/__tests__/dynamic-routes.test.ts (8 tests) 7ms
 ✓ src/router/__tests__/index.test.ts (5 tests) 4ms
 ✓ src/utils/__tests__/directives.test.ts (3 tests) 7ms

 Test Files  26 passed (26)
      Tests  141 passed (141)
   Start at  15:21:51
   Duration  28.39s (transform 24.48s, setup 0ms, collect 73.65s, tests 4.56s, environment 137.40s, prepare 21.71s)

 % Coverage report from v8
-------------------|---------|----------|---------|---------|-------------------
File               | % Stmts | % Branch | % Funcs | % Lines | Uncovered Line #s 
-------------------|---------|----------|---------|---------|-------------------
All files          |   77.58 |    79.35 |   71.11 |   77.58 |                   
 api               |   93.11 |    96.77 |   92.06 |   93.11 |                   
  System.ts        |      76 |      100 |    62.5 |      76 | 25-26,29-30,33-34 
  alarm.ts         |     100 |      100 |     100 |     100 |                   
  ...Management.ts |     100 |    94.44 |     100 |     100 | 56                
  ...Management.ts |     100 |      100 |     100 |     100 |                   
  homepage.ts      |     100 |      100 |     100 |     100 |                   
  ...Management.ts |     100 |      100 |     100 |     100 |                   
  user.ts          |   71.79 |    85.71 |      75 |   71.79 | 50-59,66-67       
  ...Management.ts |     100 |      100 |     100 |     100 |                   
 api/Statistic     |     100 |      100 |     100 |     100 |                   
  overview.ts      |     100 |      100 |     100 |     100 |                   
 composables       |   69.67 |    82.35 |    62.5 |   69.67 |                   
  useTableData.ts  |   61.11 |    78.94 |      50 |   61.11 | ...90-314,320-325 
  useWebSocket.ts  |     100 |    86.66 |     100 |     100 | 44-45             
 router            |     100 |      100 |       0 |     100 |                   
  ...mic-routes.ts |     100 |      100 |       0 |     100 |                   
  guard.ts         |     100 |      100 |     100 |     100 |                   
  index.ts         |     100 |      100 |     100 |     100 |                   
  static-routes.ts |     100 |      100 |       0 |     100 |                   
 stores            |    64.5 |    70.73 |   80.95 |    64.5 |                   
  global.ts        |     100 |      100 |     100 |     100 |                   
  ruleChain.ts     |    96.9 |    71.79 |   84.21 |    96.9 | ...,91-92,135-136 
  user.ts          |       0 |        0 |       0 |       0 | 1-145             
 utils             |   68.55 |    70.31 |   79.24 |   68.55 |                   
  common.ts        |   92.85 |    95.45 |     100 |   92.85 | 41-42             
  csv.ts           |     100 |    83.33 |     100 |     100 | 2                 
  date.ts          |     100 |      100 |     100 |     100 |                   
  responsive.ts    |   93.93 |     87.5 |     100 |   93.93 | 26-27             
  websocket.ts     |   63.94 |    60.67 |   71.79 |   63.94 | ...35-736,741-743 
-------------------|---------|----------|---------|---------|-------------------

Browserslist: browsers data (caniuse-lite) is 10 months old. Please run:
  npx update-browserslist-db@latest
  Why you should do it regularly: https://github.com/browserslist/update-db#readme
```

构建验证

- 结果：PASS 通过
- 退出码：0
- 判定依据：命令执行成功
- 检查说明：验证项目是否可以成功构建。
- 规则/阈值：构建命令非 0 退出则判定为失败。

命令：`pnpm build:check`

```text
> voltage-edge-ems@0.0.0 build:check D:\RushRush\VoltageEMS\apps
> vite build

vite v7.3.2 building client environment for production...
transforming...
✓ 2402 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                                    1.32 kB │ gzip:     0.68 kB
dist/images/home-PV-hsbJKaZM.svg                                   4.30 kB │ gzip:     1.13 kB
dist/images/CoolantTemp-CLicKI9I.svg                               4.51 kB │ gzip:     1.19 kB
dist/images/Power-C2dE89lI.svg                                     4.66 kB │ gzip:     1.24 kB
dist/images/DGEnergy-CCw7SK2h.svg                                  4.75 kB │ gzip:     1.29 kB
dist/images/home-saving-D94I-a5I.svg                               5.29 kB │ gzip:     1.61 kB
dist/images/sunny-B1J38dV6.svg                                     9.88 kB │ gzip:     2.88 kB
dist/images/jiantou-qr3YEwOm.png                                  15.26 kB
dist/images/jiantou-copy-7D_beVcw.svg                             20.40 kB │ gzip:    15.37 kB
dist/images/simple-bg-BT0b9kR1.png                               102.36 kB
dist/fonts/Montserrat-VariableFont_wght-DZEFCB5D.woff2           197.79 kB
dist/fonts/Montserrat-Italic-VariableFont_wght-BGFV_P04.woff2    213.72 kB
dist/fonts/Arimo-VariableFont_wght-UDFkYScb.woff2                215.18 kB
dist/images/device-BMS-aRvo-jHh.svg                              215.52 kB │ gzip:   161.76 kB
dist/fonts/Arimo-Italic-VariableFont_wght-De6GrK0f.woff2         256.36 kB
dist/images/device-pv-K4RwbVAl.svg                               282.98 kB │ gzip:   205.20 kB
dist/images/device-PCS-D_iKnUP7.svg                              539.20 kB │ gzip:   405.01 kB
dist/images/device-diesel-NJQ3PkaG.svg                           572.92 kB │ gzip:   430.04 kB
dist/images/home-bg-mYFNYsKE.png                                 584.23 kB
dist/images/DieselGenerator-bg-kxLmY95L.png                    1,109.55 kB
dist/images/PV-bg-DzZXfm1Z.png                                 1,186.44 kB
dist/images/battery-bg-C1f_0acD.png                            1,458.26 kB
dist/images/login-bg-DYG8vysq.png                              1,484.42 kB
dist/images/device-battery-BObDiC7e.png                        1,667.53 kB
dist/images/tuopu-bg-CBMdDRK9.png                              2,116.35 kB
dist/images/home-tuopu-B89IrEBr.svg                            2,899.69 kB │ gzip: 2,116.11 kB
dist/images/device-battery-DlXJ_uEw.svg                        4,306.16 kB │ gzip: 1,551.76 kB
dist/css/PVValueMonitoring-CtEM7coq.css                            0.18 kB │ gzip:     0.13 kB
dist/css/DieselValueMonitoring-pqL9HiRF.css                        0.18 kB │ gzip:     0.13 kB
dist/css/IconButton-BI_R10uQ.css                                   0.26 kB │ gzip:     0.17 kB
dist/css/BatteryValue-i-sm3ocn.css                                 0.27 kB │ gzip:     0.14 kB
dist/css/index-DKzQDlWY.css                                        0.29 kB │ gzip:     0.15 kB
dist/css/index-DC20yCxv.css                                        0.29 kB │ gzip:     0.15 kB
dist/css/FormDialog-DhSemn5B.css                                   0.38 kB │ gzip:     0.21 kB
dist/css/DeviceMonitoringTable-hW1iNbzG.css                        0.46 kB │ gzip:     0.21 kB
dist/css/OperationLog-CFBh3HWp.css                                 0.60 kB │ gzip:     0.23 kB
dist/css/DoughnutChart-DEprVLF5.css                                0.95 kB │ gzip:     0.31 kB
dist/css/DieselOverview-DNmuPwgQ.css                               1.14 kB │ gzip:     0.43 kB
dist/css/index-BO1tepj8.css                                        1.17 kB │ gzip:     0.32 kB
dist/css/index-BeTCx6Nb.css                                        1.21 kB │ gzip:     0.33 kB
dist/css/LoadingBg-Zga3Oqoo.css                                    1.35 kB │ gzip:     0.39 kB
dist/css/icon-pv-energy-Db6uNnOd.css                               1.79 kB │ gzip:     0.41 kB
dist/css/RuningLog-BPE6-xis.css                                    1.93 kB │ gzip:     0.56 kB
dist/css/Curves-i8gEVh1e.css                                       2.14 kB │ gzip:     0.50 kB
dist/css/lineChart-CjhPjw3D.css                                    2.29 kB │ gzip:     0.50 kB
dist/css/BatteryOverview-C6rGVq6e.css                              2.43 kB │ gzip:     0.77 kB
dist/css/ModuleCard-zdxInxDe.css                                   2.66 kB │ gzip:     1.06 kB
dist/css/PVCard-CS_H6MBy.css                                       2.70 kB │ gzip:     0.62 kB
dist/css/index-C48tPS5_.css                                        2.75 kB │ gzip:     0.53 kB
dist/css/index-DbKJszKC.css                                        2.75 kB │ gzip:     0.53 kB
dist/css/index-BqRmj2eW.css                                        2.79 kB │ gzip:     0.54 kB
dist/css/index-BbFBg9yC.css                                        2.83 kB │ gzip:     1.35 kB
dist/css/index-CRfQZwp-.css                                        2.95 kB │ gzip:     0.46 kB
dist/css/index-BVjwHH11.css                                        3.82 kB │ gzip:     0.72 kB
dist/css/PVOverview-Bpo7gCSA.css                                   4.24 kB │ gzip:     0.96 kB
dist/css/index-BxiWW7kJ.css                                        4.36 kB │ gzip:     0.77 kB
dist/css/index-Ze3NeiMV.css                                        4.36 kB │ gzip:     0.77 kB
dist/css/BatteryManagement-DAY63Jy0.css                            5.08 kB │ gzip:     0.77 kB
dist/css/Overview-BdOn0fdW.css                                     5.33 kB │ gzip:     0.91 kB
dist/css/vendor-1WlF9NVH.css                                       6.22 kB │ gzip:     1.39 kB
dist/css/index-DHcuQmKi.css                                        7.40 kB │ gzip:     1.33 kB
dist/css/index-C4qBV3I3.css                                       11.30 kB │ gzip:     2.19 kB
dist/css/MainLayout-D2JNpG7f.css                                  16.06 kB │ gzip:     6.14 kB
dist/css/index-CGw0v_xD.css                                      104.41 kB │ gzip:    13.31 kB
dist/css/element-plus-BceCcFGP.css                               410.36 kB │ gzip:    50.87 kB
dist/js/_plugin-vue_export-helper-DlAUqK2U.js                      0.09 kB │ gzip:     0.10 kB
dist/js/channelsManagement-CxdY7Btv.js                             0.17 kB │ gzip:     0.15 kB
dist/js/alarm-C6l1yYpg.js                                          0.28 kB │ gzip:     0.18 kB
dist/js/card-icon-B7zR7iIw.js                                      0.37 kB │ gzip:     0.29 kB
dist/js/common-DiW0yPUS.js                                         0.38 kB │ gzip:     0.25 kB
dist/js/alarm-export-wcV7bbTk.js                                   0.49 kB │ gzip:     0.36 kB
dist/js/useWebSocket-BzEnGtMX.js                                   0.68 kB │ gzip:     0.38 kB
dist/js/IconButton-C5wSlWYo.js                                     0.72 kB │ gzip:     0.45 kB
dist/js/ModuleCard-C3M-_SzW.js                                     0.81 kB │ gzip:     0.48 kB
dist/js/index-Dvouvp_k.js                                          0.82 kB │ gzip:     0.49 kB
dist/js/index-ClkpWAWs.js                                          0.82 kB │ gzip:     0.49 kB
dist/js/PVCard-XS1rMHOB.js                                         1.00 kB │ gzip:     0.52 kB
dist/js/FormDialog-DABkCyu9.js                                     1.18 kB │ gzip:     0.67 kB
dist/js/index-DqZGoIiv.js                                          1.34 kB │ gzip:     0.67 kB
dist/js/index-NOjgJ_Jt.js                                          1.38 kB │ gzip:     0.67 kB
dist/js/OperationLog-D-CRVsEB.js                                   1.44 kB │ gzip:     0.71 kB
dist/js/DieselOverview-CL3ROa-_.js                                 1.53 kB │ gzip:     0.87 kB
dist/js/LoadingBg-BT_FPPBz.js                                      1.59 kB │ gzip:     0.81 kB
dist/js/user-add-CmQPpnDU.js                                       1.65 kB │ gzip:     0.55 kB
dist/js/table-search-Bs_lYf8J.js                                   1.73 kB │ gzip:     0.70 kB
dist/js/PVValueMonitoring-BjSUzcar.js                              1.73 kB │ gzip:     0.87 kB
dist/js/DieselValueMonitoring-NyG5CKcu.js                          1.73 kB │ gzip:     0.87 kB
dist/js/index-BbB4HEd2.js                                          1.76 kB │ gzip:     0.75 kB
dist/js/index-D6SZjJis.js                                          2.05 kB │ gzip:     1.03 kB
dist/js/BatteryOverview-DSjwM2Ix.js                                2.25 kB │ gzip:     1.06 kB
dist/js/alarm-history-BtaJmvea.js                                  2.27 kB │ gzip:     0.83 kB
dist/js/RuningLog-DwFMgWDM.js                                      2.30 kB │ gzip:     1.04 kB
dist/js/DeviceMonitoringTable-CptvDajJ.js                          2.60 kB │ gzip:     0.90 kB
dist/js/sunny-D4O82m9v.js                                          2.62 kB │ gzip:     1.05 kB
dist/js/useTableData-Bo-Qh8LY.js                                   2.67 kB │ gzip:     1.26 kB
dist/js/BatteryValue-C2EnavDa.js                                   2.91 kB │ gzip:     1.07 kB
dist/js/PVOverview-BFZpQxW-.js                                     3.07 kB │ gzip:     1.41 kB
dist/js/BatteryManagement-D0JwsB2i.js                              3.33 kB │ gzip:     0.85 kB
dist/js/index-CNvDZpL9.js                                          3.53 kB │ gzip:     1.56 kB
dist/js/index-D69FAhLY.js                                          3.54 kB │ gzip:     1.56 kB
dist/js/Curves-DIrQXKyQ.js                                         4.08 kB │ gzip:     1.71 kB
dist/js/Current-7Vei_IK1.js                                        4.46 kB │ gzip:     1.08 kB
dist/js/Oil-DMwDiJfy.js                                            4.76 kB │ gzip:     1.18 kB
dist/js/MainLayout-De7m7orB.js                                     5.02 kB │ gzip:     1.87 kB
dist/js/Voltage-Cni3kFdL.js                                        5.11 kB │ gzip:     1.28 kB
dist/js/icon-pv-energy-CTMzVHgI.js                                 5.65 kB │ gzip:     1.64 kB
dist/js/index-DYstQwXk.js                                          5.87 kB │ gzip:     2.14 kB
dist/js/Overview-Cj5C7buy.js                                       6.31 kB │ gzip:     2.53 kB
dist/js/DoughnutChart-RV4Py0D3.js                                  7.44 kB │ gzip:     2.66 kB
dist/js/index-CH8hRsgp.js                                          8.13 kB │ gzip:     3.07 kB
dist/js/index-Dq_uvwb8.js                                         13.95 kB │ gzip:     4.46 kB
dist/js/index-8hmxkrAW.js                                         13.95 kB │ gzip:     4.46 kB
dist/js/lineChart-NKZnAJfD.js                                     18.62 kB │ gzip:     4.36 kB
dist/js/index-DqeBoZzD.js                                         20.39 kB │ gzip:     5.72 kB
dist/js/index-Dgl8pQta.js                                         45.24 kB │ gzip:    14.68 kB
dist/js/index-B3COSOiL.js                                         75.63 kB │ gzip:    16.63 kB
dist/js/echarts-DYZx-odV.js                                      443.51 kB │ gzip:   150.02 kB
dist/js/vendor-BHdEalbJ.js                                       466.43 kB │ gzip:   165.51 kB
dist/js/element-plus-yg8aBi5k.js                                 756.75 kB │ gzip:   238.58 kB
✓ built in 21.35s

✨ [vite-plugin-compression]:algorithm=gzip - compressed file successfully: 
dist/D:/RushRush/VoltageEMS/apps/css/MainLayout-D2JNpG7f.css.gz                                15.68kb / gzip: 5.98kb
dist/D:/RushRush/VoltageEMS/apps/css/index-C4qBV3I3.css.gz                                     11.04kb / gzip: 2.12kb
dist/D:/RushRush/VoltageEMS/apps/js/index-8hmxkrAW.js.gz                                       13.63kb / gzip: 4.35kb
dist/D:/RushRush/VoltageEMS/apps/js/index-Dgl8pQta.js.gz                                       44.18kb / gzip: 14.33kb
dist/D:/RushRush/VoltageEMS/apps/js/index-Dq_uvwb8.js.gz                                       13.62kb / gzip: 4.35kb
dist/D:/RushRush/VoltageEMS/apps/js/index-DqeBoZzD.js.gz                                       19.91kb / gzip: 5.58kb
dist/D:/RushRush/VoltageEMS/apps/css/index-CGw0v_xD.css.gz                                     101.96kb / gzip: 12.82kb
dist/D:/RushRush/VoltageEMS/apps/js/index-B3COSOiL.js.gz                                       73.85kb / gzip: 16.14kb
dist/D:/RushRush/VoltageEMS/apps/js/lineChart-NKZnAJfD.js.gz                                   18.18kb / gzip: 4.23kb
dist/D:/RushRush/VoltageEMS/apps/css/element-plus-BceCcFGP.css.gz                              400.74kb / gzip: 49.15kb
dist/D:/RushRush/VoltageEMS/apps/fonts/Montserrat-VariableFont_wght-DZEFCB5D.woff2.gz          193.16kb / gzip: 193.19kb
dist/D:/RushRush/VoltageEMS/apps/fonts/Arimo-VariableFont_wght-UDFkYScb.woff2.gz               210.14kb / gzip: 209.23kb
dist/D:/RushRush/VoltageEMS/apps/fonts/Montserrat-Italic-VariableFont_wght-BGFV_P04.woff2.gz   208.71kb / gzip: 208.75kb
dist/D:/RushRush/VoltageEMS/apps/fonts/Arimo-Italic-VariableFont_wght-De6GrK0f.woff2.gz        250.36kb / gzip: 249.33kb
dist/D:/RushRush/VoltageEMS/apps/js/vendor-BHdEalbJ.js.gz                                      455.50kb / gzip: 161.44kb
dist/D:/RushRush/VoltageEMS/apps/js/echarts-DYZx-odV.js.gz                                     433.11kb / gzip: 146.25kb
dist/D:/RushRush/VoltageEMS/apps/js/element-plus-yg8aBi5k.js.gz                                739.01kb / gzip: 232.68kb



NODE_ENV=production is not supported in the .env file. Only NODE_ENV=development is supported to create a development build of your project. If you need to set process.env.NODE_ENV, you can set it in the Vite config instead.
Browserslist: browsers data (caniuse-lite) is 10 months old. Please run:
  npx update-browserslist-db@latest
  Why you should do it regularly: https://github.com/browserslist/update-db#readme
```

构建产物体积

- 结果：WARN 警告
- 退出码：0
- 判定依据：构建已完成，但产物体积超出建议阈值
- 检查说明：分析 dist 目录产物大小，识别过大的 JS/CSS 资源。
- 规则/阈值：脚本能分析 dist 即退出码为 0；若单个 JS 超过 800 KB、单个 CSS 超过 450 KB 或 dist 总体积超过 15 MB，则记为警告。

关键结论：

- dist 总大小：21562.37 KB（建议不超过 15360.00 KB）
- JS 资源数量：54
- CSS 资源数量：38
- 最大 JS 阈值：800.00 KB，超标数量：0
- 最大 CSS 阈值：450.00 KB，超标数量：0
- 体积最大的 5 个产物：images/device-battery-DlXJ_uEw.svg：4205.24 KB；images/home-tuopu-B89IrEBr.svg：2831.73 KB；images/tuopu-bg-CBMdDRK9.png：2066.75 KB；images/device-battery-BObDiC7e.png：1628.44 KB；images/login-bg-DYG8vysq.png：1449.63 KB

命令：`pnpm build:check`

```text
> voltage-edge-ems@0.0.0 build:check D:\RushRush\VoltageEMS\apps
> vite build

vite v7.3.2 building client environment for production...
transforming...
✓ 2402 modules transformed.
rendering chunks...
computing gzip size...
dist/index.html                                                    1.32 kB │ gzip:     0.68 kB
dist/images/home-PV-hsbJKaZM.svg                                   4.30 kB │ gzip:     1.13 kB
dist/images/CoolantTemp-CLicKI9I.svg                               4.51 kB │ gzip:     1.19 kB
dist/images/Power-C2dE89lI.svg                                     4.66 kB │ gzip:     1.24 kB
dist/images/DGEnergy-CCw7SK2h.svg                                  4.75 kB │ gzip:     1.29 kB
dist/images/home-saving-D94I-a5I.svg                               5.29 kB │ gzip:     1.61 kB
dist/images/sunny-B1J38dV6.svg                                     9.88 kB │ gzip:     2.88 kB
dist/images/jiantou-qr3YEwOm.png                                  15.26 kB
dist/images/jiantou-copy-7D_beVcw.svg                             20.40 kB │ gzip:    15.37 kB
dist/images/simple-bg-BT0b9kR1.png                               102.36 kB
dist/fonts/Montserrat-VariableFont_wght-DZEFCB5D.woff2           197.79 kB
dist/fonts/Montserrat-Italic-VariableFont_wght-BGFV_P04.woff2    213.72 kB
dist/fonts/Arimo-VariableFont_wght-UDFkYScb.woff2                215.18 kB
dist/images/device-BMS-aRvo-jHh.svg                              215.52 kB │ gzip:   161.76 kB
dist/fonts/Arimo-Italic-VariableFont_wght-De6GrK0f.woff2         256.36 kB
dist/images/device-pv-K4RwbVAl.svg                               282.98 kB │ gzip:   205.20 kB
dist/images/device-PCS-D_iKnUP7.svg                              539.20 kB │ gzip:   405.01 kB
dist/images/device-diesel-NJQ3PkaG.svg                           572.92 kB │ gzip:   430.04 kB
dist/images/home-bg-mYFNYsKE.png                                 584.23 kB
dist/images/DieselGenerator-bg-kxLmY95L.png                    1,109.55 kB
dist/images/PV-bg-DzZXfm1Z.png                                 1,186.44 kB
dist/images/battery-bg-C1f_0acD.png                            1,458.26 kB
dist/images/login-bg-DYG8vysq.png                              1,484.42 kB
dist/images/device-battery-BObDiC7e.png                        1,667.53 kB
dist/images/tuopu-bg-CBMdDRK9.png                              2,116.35 kB
dist/images/home-tuopu-B89IrEBr.svg                            2,899.69 kB │ gzip: 2,116.11 kB
dist/images/device-battery-DlXJ_uEw.svg                        4,306.16 kB │ gzip: 1,551.76 kB
dist/css/PVValueMonitoring-CtEM7coq.css                            0.18 kB │ gzip:     0.13 kB
dist/css/DieselValueMonitoring-pqL9HiRF.css                        0.18 kB │ gzip:     0.13 kB
dist/css/IconButton-BI_R10uQ.css                                   0.26 kB │ gzip:     0.17 kB
dist/css/BatteryValue-i-sm3ocn.css                                 0.27 kB │ gzip:     0.14 kB
dist/css/index-DKzQDlWY.css                                        0.29 kB │ gzip:     0.15 kB
dist/css/index-DC20yCxv.css                                        0.29 kB │ gzip:     0.15 kB
dist/css/FormDialog-DhSemn5B.css                                   0.38 kB │ gzip:     0.21 kB
dist/css/DeviceMonitoringTable-hW1iNbzG.css                        0.46 kB │ gzip:     0.21 kB
dist/css/OperationLog-CFBh3HWp.css                                 0.60 kB │ gzip:     0.23 kB
dist/css/DoughnutChart-DEprVLF5.css                                0.95 kB │ gzip:     0.31 kB
dist/css/DieselOverview-DNmuPwgQ.css                               1.14 kB │ gzip:     0.43 kB
dist/css/index-BO1tepj8.css                                        1.17 kB │ gzip:     0.32 kB
dist/css/index-BeTCx6Nb.css                                        1.21 kB │ gzip:     0.33 kB
dist/css/LoadingBg-Zga3Oqoo.css                                    1.35 kB │ gzip:     0.39 kB
dist/css/icon-pv-energy-Db6uNnOd.css                               1.79 kB │ gzip:     0.41 kB
dist/css/RuningLog-BPE6-xis.css                                    1.93 kB │ gzip:     0.56 kB
dist/css/Curves-i8gEVh1e.css                                       2.14 kB │ gzip:     0.50 kB
dist/css/lineChart-CjhPjw3D.css                                    2.29 kB │ gzip:     0.50 kB
dist/css/BatteryOverview-C6rGVq6e.css                              2.43 kB │ gzip:     0.77 kB
dist/css/ModuleCard-zdxInxDe.css                                   2.66 kB │ gzip:     1.06 kB
dist/css/PVCard-CS_H6MBy.css                                       2.70 kB │ gzip:     0.62 kB
dist/css/index-C48tPS5_.css                                        2.75 kB │ gzip:     0.53 kB
dist/css/index-DbKJszKC.css                                        2.75 kB │ gzip:     0.53 kB
dist/css/index-BqRmj2eW.css                                        2.79 kB │ gzip:     0.54 kB
dist/css/index-BbFBg9yC.css                                        2.83 kB │ gzip:     1.35 kB
dist/css/index-CRfQZwp-.css                                        2.95 kB │ gzip:     0.46 kB
dist/css/index-BVjwHH11.css                                        3.82 kB │ gzip:     0.72 kB
dist/css/PVOverview-Bpo7gCSA.css                                   4.24 kB │ gzip:     0.96 kB
dist/css/index-BxiWW7kJ.css                                        4.36 kB │ gzip:     0.77 kB
dist/css/index-Ze3NeiMV.css                                        4.36 kB │ gzip:     0.77 kB
dist/css/BatteryManagement-DAY63Jy0.css                            5.08 kB │ gzip:     0.77 kB
dist/css/Overview-BdOn0fdW.css                                     5.33 kB │ gzip:     0.91 kB
dist/css/vendor-1WlF9NVH.css                                       6.22 kB │ gzip:     1.39 kB
dist/css/index-DHcuQmKi.css                                        7.40 kB │ gzip:     1.33 kB
dist/css/index-C4qBV3I3.css                                       11.30 kB │ gzip:     2.19 kB
dist/css/MainLayout-D2JNpG7f.css                                  16.06 kB │ gzip:     6.14 kB
dist/css/index-CGw0v_xD.css                                      104.41 kB │ gzip:    13.31 kB
dist/css/element-plus-BceCcFGP.css                               410.36 kB │ gzip:    50.87 kB
dist/js/_plugin-vue_export-helper-DlAUqK2U.js                      0.09 kB │ gzip:     0.10 kB
dist/js/channelsManagement-CxdY7Btv.js                             0.17 kB │ gzip:     0.15 kB
dist/js/alarm-C6l1yYpg.js                                          0.28 kB │ gzip:     0.18 kB
dist/js/card-icon-B7zR7iIw.js                                      0.37 kB │ gzip:     0.29 kB
dist/js/common-DiW0yPUS.js                                         0.38 kB │ gzip:     0.25 kB
dist/js/alarm-export-wcV7bbTk.js                                   0.49 kB │ gzip:     0.36 kB
dist/js/useWebSocket-BzEnGtMX.js                                   0.68 kB │ gzip:     0.38 kB
dist/js/IconButton-C5wSlWYo.js                                     0.72 kB │ gzip:     0.45 kB
dist/js/ModuleCard-C3M-_SzW.js                                     0.81 kB │ gzip:     0.48 kB
dist/js/index-Dvouvp_k.js                                          0.82 kB │ gzip:     0.49 kB
dist/js/index-ClkpWAWs.js                                          0.82 kB │ gzip:     0.49 kB
dist/js/PVCard-XS1rMHOB.js                                         1.00 kB │ gzip:     0.52 kB
dist/js/FormDialog-DABkCyu9.js                                     1.18 kB │ gzip:     0.67 kB
dist/js/index-DqZGoIiv.js                                          1.34 kB │ gzip:     0.67 kB
dist/js/index-NOjgJ_Jt.js                                          1.38 kB │ gzip:     0.67 kB
dist/js/OperationLog-D-CRVsEB.js                                   1.44 kB │ gzip:     0.71 kB
dist/js/DieselOverview-CL3ROa-_.js                                 1.53 kB │ gzip:     0.87 kB
dist/js/LoadingBg-BT_FPPBz.js                                      1.59 kB │ gzip:     0.81 kB
dist/js/user-add-CmQPpnDU.js                                       1.65 kB │ gzip:     0.55 kB
dist/js/table-search-Bs_lYf8J.js                                   1.73 kB │ gzip:     0.70 kB
dist/js/PVValueMonitoring-BjSUzcar.js                              1.73 kB │ gzip:     0.87 kB
dist/js/DieselValueMonitoring-NyG5CKcu.js                          1.73 kB │ gzip:     0.87 kB
dist/js/index-BbB4HEd2.js                                          1.76 kB │ gzip:     0.75 kB
dist/js/index-D6SZjJis.js                                          2.05 kB │ gzip:     1.03 kB
dist/js/BatteryOverview-DSjwM2Ix.js                                2.25 kB │ gzip:     1.06 kB
dist/js/alarm-history-BtaJmvea.js                                  2.27 kB │ gzip:     0.83 kB
dist/js/RuningLog-DwFMgWDM.js                                      2.30 kB │ gzip:     1.04 kB
dist/js/DeviceMonitoringTable-CptvDajJ.js                          2.60 kB │ gzip:     0.90 kB
dist/js/sunny-D4O82m9v.js                                          2.62 kB │ gzip:     1.05 kB
dist/js/useTableData-Bo-Qh8LY.js                                   2.67 kB │ gzip:     1.26 kB
dist/js/BatteryValue-C2EnavDa.js                                   2.91 kB │ gzip:     1.07 kB
dist/js/PVOverview-BFZpQxW-.js                                     3.07 kB │ gzip:     1.41 kB
dist/js/BatteryManagement-D0JwsB2i.js                              3.33 kB │ gzip:     0.85 kB
dist/js/index-CNvDZpL9.js                                          3.53 kB │ gzip:     1.56 kB
dist/js/index-D69FAhLY.js                                          3.54 kB │ gzip:     1.56 kB
dist/js/Curves-DIrQXKyQ.js                                         4.08 kB │ gzip:     1.71 kB
dist/js/Current-7Vei_IK1.js                                        4.46 kB │ gzip:     1.08 kB
dist/js/Oil-DMwDiJfy.js                                            4.76 kB │ gzip:     1.18 kB
dist/js/MainLayout-De7m7orB.js                                     5.02 kB │ gzip:     1.87 kB
dist/js/Voltage-Cni3kFdL.js                                        5.11 kB │ gzip:     1.28 kB
dist/js/icon-pv-energy-CTMzVHgI.js                                 5.65 kB │ gzip:     1.64 kB
dist/js/index-DYstQwXk.js                                          5.87 kB │ gzip:     2.14 kB
dist/js/Overview-Cj5C7buy.js                                       6.31 kB │ gzip:     2.53 kB
dist/js/DoughnutChart-RV4Py0D3.js                                  7.44 kB │ gzip:     2.66 kB
dist/js/index-CH8hRsgp.js                                          8.13 kB │ gzip:     3.07 kB
dist/js/index-Dq_uvwb8.js                                         13.95 kB │ gzip:     4.46 kB
dist/js/index-8hmxkrAW.js                                         13.95 kB │ gzip:     4.46 kB
dist/js/lineChart-NKZnAJfD.js                                     18.62 kB │ gzip:     4.36 kB
dist/js/index-DqeBoZzD.js                                         20.39 kB │ gzip:     5.72 kB
dist/js/index-Dgl8pQta.js                                         45.24 kB │ gzip:    14.68 kB
dist/js/index-B3COSOiL.js                                         75.63 kB │ gzip:    16.63 kB
dist/js/echarts-DYZx-odV.js                                      443.51 kB │ gzip:   150.02 kB
dist/js/vendor-BHdEalbJ.js                                       466.43 kB │ gzip:   165.51 kB
dist/js/element-plus-yg8aBi5k.js                                 756.75 kB │ gzip:   238.58 kB
✓ built in 21.35s

✨ [vite-plugin-compression]:algorithm=gzip - compressed file successfully: 
dist/D:/RushRush/VoltageEMS/apps/css/MainLayout-D2JNpG7f.css.gz                                15.68kb / gzip: 5.98kb
dist/D:/RushRush/VoltageEMS/apps/css/index-C4qBV3I3.css.gz                                     11.04kb / gzip: 2.12kb
dist/D:/RushRush/VoltageEMS/apps/js/index-8hmxkrAW.js.gz                                       13.63kb / gzip: 4.35kb
dist/D:/RushRush/VoltageEMS/apps/js/index-Dgl8pQta.js.gz                                       44.18kb / gzip: 14.33kb
dist/D:/RushRush/VoltageEMS/apps/js/index-Dq_uvwb8.js.gz                                       13.62kb / gzip: 4.35kb
dist/D:/RushRush/VoltageEMS/apps/js/index-DqeBoZzD.js.gz                                       19.91kb / gzip: 5.58kb
dist/D:/RushRush/VoltageEMS/apps/css/index-CGw0v_xD.css.gz                                     101.96kb / gzip: 12.82kb
dist/D:/RushRush/VoltageEMS/apps/js/index-B3COSOiL.js.gz                                       73.85kb / gzip: 16.14kb
dist/D:/RushRush/VoltageEMS/apps/js/lineChart-NKZnAJfD.js.gz                                   18.18kb / gzip: 4.23kb
dist/D:/RushRush/VoltageEMS/apps/css/element-plus-BceCcFGP.css.gz                              400.74kb / gzip: 49.15kb
dist/D:/RushRush/VoltageEMS/apps/fonts/Montserrat-VariableFont_wght-DZEFCB5D.woff2.gz          193.16kb / gzip: 193.19kb
dist/D:/RushRush/VoltageEMS/apps/fonts/Arimo-VariableFont_wght-UDFkYScb.woff2.gz               210.14kb / gzip: 209.23kb
dist/D:/RushRush/VoltageEMS/apps/fonts/Montserrat-Italic-VariableFont_wght-BGFV_P04.woff2.gz   208.71kb / gzip: 208.75kb
dist/D:/RushRush/VoltageEMS/apps/fonts/Arimo-Italic-VariableFont_wght-De6GrK0f.woff2.gz        250.36kb / gzip: 249.33kb
dist/D:/RushRush/VoltageEMS/apps/js/vendor-BHdEalbJ.js.gz                                      455.50kb / gzip: 161.44kb
dist/D:/RushRush/VoltageEMS/apps/js/echarts-DYZx-odV.js.gz                                     433.11kb / gzip: 146.25kb
dist/D:/RushRush/VoltageEMS/apps/js/element-plus-yg8aBi5k.js.gz                                739.01kb / gzip: 232.68kb



NODE_ENV=production is not supported in the .env file. Only NODE_ENV=development is supported to create a development build of your project. If you need to set process.env.NODE_ENV, you can set it in the Vite config instead.
Browserslist: browsers data (caniuse-lite) is 10 months old. Please run:
  npx update-browserslist-db@latest
  Why you should do it regularly: https://github.com/browserslist/update-db#readme
```

依赖安全扫描

- 结果：PASS 通过
- 退出码：1
- 判定依据：未检测到高危、严重或中危漏洞
- 检查说明：通过 npm audit 检查高危与严重依赖漏洞。
- 规则/阈值：严重或高危漏洞记为失败；中危漏洞记为警告。当前 failOn=critical, high，warnOn=moderate。

关键结论：

- critical：0
- high：0
- moderate：0
- low：0

命令：`npm audit --json --audit-level=moderate --registry=https://registry.npmjs.org`

```text
{
  "auditReportVersion": 2,
  "vulnerabilities": {
    "ajv": {
      "name": "ajv",
      "severity": "moderate",
      "isDirect": false,
      "via": [
        {
          "source": 1113714,
          "name": "ajv",
          "dependency": "ajv",
          "title": "ajv has ReDoS when using `$data` option",
          "url": "https://github.com/advisories/GHSA-2g4f-4pwh-qvx6",
          "severity": "moderate",
          "cwe": [
            "CWE-400",
            "CWE-1333"
          ],
          "cvss": {
            "score": 0,
            "vectorString": null
          },
          "range": "<6.14.0"
        }
      ],
      "effects": [],
      "range": "<6.14.0",
      "nodes": [
        "node_modules/ajv"
      ],
      "fixAvailable": true
    },
    "brace-expansion": {
      "name": "brace-expansion",
      "severity": "moderate",
      "isDirect": false,
      "via": [
        {
          "source": 1115540,
          "name": "brace-expansion",
          "dependency": "brace-expansion",
          "title": "brace-expansion: Zero-step sequence causes process hang and memory exhaustion",
          "url": "https://github.com/advisories/GHSA-f886-m6hf-6m8v",
          "severity": "moderate",
          "cwe": [
            "CWE-400"
          ],
          "cvss": {
            "score": 6.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:U/C:N/I:N/A:H"
          },
          "range": "<1.1.13"
        },
        {
          "source": 1115541,
          "name": "brace-expansion",
          "dependency": "brace-expansion",
          "title": "brace-expansion: Zero-step sequence causes process hang and memory exhaustion",
          "url": "https://github.com/advisories/GHSA-f886-m6hf-6m8v",
          "severity": "moderate",
          "cwe": [
            "CWE-400"
          ],
          "cvss": {
            "score": 6.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:U/C:N/I:N/A:H"
          },
          "range": ">=2.0.0 <2.0.3"
        }
      ],
      "effects": [],
      "range": "<1.1.13 || >=2.0.0 <2.0.3",
      "nodes": [
        "node_modules/@eslint/config-array/node_modules/brace-expansion",
        "node_modules/@eslint/eslintrc/node_modules/brace-expansion",
        "node_modules/brace-expansion",
        "node_modules/eslint/node_modules/brace-expansion"
      ],
      "fixAvailable": true
    },
    "defu": {
      "name": "defu",
      "severity": "high",
      "isDirect": false,
      "via": [
        {
          "source": 1116102,
          "name": "defu",
          "dependency": "defu",
          "title": "defu: Prototype pollution via `__proto__` key in defaults argument",
          "url": "https://github.com/advisories/GHSA-737v-mqg7-c878",
          "severity": "high",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 7.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:H/A:N"
          },
          "range": "<=6.1.4"
        }
      ],
      "effects": [],
      "range": "<=6.1.4",
      "nodes": [
        "node_modules/defu"
      ],
      "fixAvailable": true
    },
    "editorconfig": {
      "name": "editorconfig",
      "severity": "high",
      "isDirect": false,
      "via": [
        "minimatch"
      ],
      "effects": [],
      "range": "1.0.3 - 1.0.4 || 2.0.0",
      "nodes": [
        "node_modules/editorconfig"
      ],
      "fixAvailable": true
    },
    "flatted": {
      "name": "flatted",
      "severity": "high",
      "isDirect": false,
      "via": [
        {
          "source": 1114526,
          "name": "flatted",
          "dependency": "flatted",
          "title": "flatted vulnerable to unbounded recursion DoS in parse() revive phase",
          "url": "https://github.com/advisories/GHSA-25h7-pfq9-p65f",
          "severity": "high",
          "cwe": [
            "CWE-674"
          ],
          "cvss": {
            "score": 7.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"
          },
          "range": "<3.4.0"
        },
        {
          "source": 1115357,
          "name": "flatted",
          "dependency": "flatted",
          "title": "Prototype Pollution via parse() in NodeJS flatted",
          "url": "https://github.com/advisories/GHSA-rf6f-7fwh-wjgh",
          "severity": "high",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 0,
            "vectorString": null
          },
          "range": "<=3.4.1"
        }
      ],
      "effects": [],
      "range": "<=3.4.1",
      "nodes": [
        "node_modules/flatted"
      ],
      "fixAvailable": true
    },
    "glob": {
      "name": "glob",
      "severity": "high",
      "isDirect": false,
      "via": [
        {
          "source": 1109842,
          "name": "glob",
          "dependency": "glob",
          "title": "glob CLI: Command injection via -c/--cmd executes matches with shell:true",
          "url": "https://github.com/advisories/GHSA-5j98-mcp5-4vw2",
          "severity": "high",
          "cwe": [
            "CWE-78"
          ],
          "cvss": {
            "score": 7.5,
            "vectorString": "CVSS:3.1/AV:N/AC:H/PR:L/UI:N/S:U/C:H/I:H/A:H"
          },
          "range": ">=10.2.0 <10.5.0"
        }
      ],
      "effects": [],
      "range": "10.2.0 - 10.4.5",
      "nodes": [
        "node_modules/glob"
      ],
      "fixAvailable": true
    },
    "immutable": {
      "name": "immutable",
      "severity": "high",
      "isDirect": false,
      "via": [
        {
          "source": 1114168,
          "name": "immutable",
          "dependency": "immutable",
          "title": "Immutable is vulnerable to Prototype Pollution",
          "url": "https://github.com/advisories/GHSA-wf6x-7x77-mvgw",
          "severity": "high",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 0,
            "vectorString": null
          },
          "range": ">=5.0.0 <5.1.5"
        }
      ],
      "effects": [],
      "range": "5.0.0 - 5.1.4",
      "nodes": [
        "node_modules/immutable"
      ],
      "fixAvailable": true
    },
    "js-yaml": {
      "name": "js-yaml",
      "severity": "moderate",
      "isDirect": false,
      "via": [
        {
          "source": 1112715,
          "name": "js-yaml",
          "dependency": "js-yaml",
          "title": "js-yaml has prototype pollution in merge (<<)",
          "url": "https://github.com/advisories/GHSA-mh29-5h37-fv8m",
          "severity": "moderate",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 5.3,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:L/A:N"
          },
          "range": ">=4.0.0 <4.1.1"
        }
      ],
      "effects": [],
      "range": "4.0.0 - 4.1.0",
      "nodes": [
        "node_modules/js-yaml"
      ],
      "fixAvailable": true
    },
    "lodash": {
      "name": "lodash",
      "severity": "high",
      "isDirect": false,
      "via": [
        {
          "source": 1112455,
          "name": "lodash",
          "dependency": "lodash",
          "title": "Lodash has Prototype Pollution Vulnerability in `_.unset` and `_.omit` functions",
          "url": "https://github.com/advisories/GHSA-xxjr-mmjv-4gpg",
          "severity": "moderate",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 6.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:L/A:L"
          },
          "range": ">=4.0.0 <=4.17.22"
        },
        {
          "source": 1115806,
          "name": "lodash",
          "dependency": "lodash",
          "title": "lodash vulnerable to Code Injection via `_.template` imports key names",
          "url": "https://github.com/advisories/GHSA-r5fr-rjxr-66jc",
          "severity": "high",
          "cwe": [
            "CWE-94"
          ],
          "cvss": {
            "score": 8.1,
            "vectorString": "CVSS:3.1/AV:N/AC:H/PR:N/UI:N/S:U/C:H/I:H/A:H"
          },
          "range": ">=4.0.0 <=4.17.23"
        },
        {
          "source": 1115810,
          "name": "lodash",
          "dependency": "lodash",
          "title": "lodash vulnerable to Prototype Pollution via array path bypass in `_.unset` and `_.omit`",
          "url": "https://github.com/advisories/GHSA-f23m-r3pf-42rh",
          "severity": "moderate",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 6.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:L/A:L"
          },
          "range": "<=4.17.23"
        }
      ],
      "effects": [],
      "range": "<=4.17.23",
      "nodes": [
        "node_modules/lodash"
      ],
      "fixAvailable": true
    },
    "lodash-es": {
      "name": "lodash-es",
      "severity": "high",
      "isDirect": false,
      "via": [
        {
          "source": 1112453,
          "name": "lodash-es",
          "dependency": "lodash-es",
          "title": "Lodash has Prototype Pollution Vulnerability in `_.unset` and `_.omit` functions",
          "url": "https://github.com/advisories/GHSA-xxjr-mmjv-4gpg",
          "severity": "moderate",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 6.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:L/A:L"
          },
          "range": ">=4.0.0 <=4.17.22"
        },
        {
          "source": 1115805,
          "name": "lodash-es",
          "dependency": "lodash-es",
          "title": "lodash vulnerable to Code Injection via `_.template` imports key names",
          "url": "https://github.com/advisories/GHSA-r5fr-rjxr-66jc",
          "severity": "high",
          "cwe": [
            "CWE-94"
          ],
          "cvss": {
            "score": 8.1,
            "vectorString": "CVSS:3.1/AV:N/AC:H/PR:N/UI:N/S:U/C:H/I:H/A:H"
          },
          "range": ">=4.0.0 <=4.17.23"
        },
        {
          "source": 1115809,
          "name": "lodash-es",
          "dependency": "lodash-es",
          "title": "lodash vulnerable to Prototype Pollution via array path bypass in `_.unset` and `_.omit`",
          "url": "https://github.com/advisories/GHSA-f23m-r3pf-42rh",
          "severity": "moderate",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 6.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:L/A:L"
          },
          "range": "<=4.17.23"
        }
      ],
      "effects": [],
      "range": "<=4.17.23",
      "nodes": [
        "node_modules/lodash-es"
      ],
      "fixAvailable": true
    },
    "minimatch": {
      "name": "minimatch",
      "severity": "high",
      "isDirect": false,
      "via": [
        {
          "source": 1113459,
          "name": "minimatch",
          "dependency": "minimatch",
          "title": "minimatch has a ReDoS via repeated wildcards with non-matching literal in pattern",
          "url": "https://github.com/advisories/GHSA-3ppc-4f35-3m26",
          "severity": "high",
          "cwe": [
            "CWE-1333"
          ],
          "cvss": {
            "score": 0,
            "vectorString": null
          },
          "range": "<3.1.3"
        },
        {
          "source": 1113465,
          "name": "minimatch",
          "dependency": "minimatch",
          "title": "minimatch has a ReDoS via repeated wildcards with non-matching literal in pattern",
          "url": "https://github.com/advisories/GHSA-3ppc-4f35-3m26",
          "severity": "high",
          "cwe": [
            "CWE-1333"
          ],
          "cvss": {
            "score": 0,
            "vectorString": null
          },
          "range": ">=9.0.0 <9.0.6"
        },
        {
          "source": 1113538,
          "name": "minimatch",
          "dependency": "minimatch",
          "title": "minimatch has ReDoS: matchOne() combinatorial backtracking via multiple non-adjacent GLOBSTAR segments",
          "url": "https://github.com/advisories/GHSA-7r86-cg39-jmmj",
          "severity": "high",
          "cwe": [
            "CWE-407"
          ],
          "cvss": {
            "score": 7.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"
          },
          "range": "<3.1.3"
        },
        {
          "source": 1113544,
          "name": "minimatch",
          "dependency": "minimatch",
          "title": "minimatch has ReDoS: matchOne() combinatorial backtracking via multiple non-adjacent GLOBSTAR segments",
          "url": "https://github.com/advisories/GHSA-7r86-cg39-jmmj",
          "severity": "high",
          "cwe": [
            "CWE-407"
          ],
          "cvss": {
            "score": 7.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"
          },
          "range": ">=9.0.0 <9.0.7"
        },
        {
          "source": 1113546,
          "name": "minimatch",
          "dependency": "minimatch",
          "title": "minimatch ReDoS: nested *() extglobs generate catastrophically backtracking regular expressions",
          "url": "https://github.com/advisories/GHSA-23c5-xmqv-rm74",
          "severity": "high",
          "cwe": [
            "CWE-1333"
          ],
          "cvss": {
            "score": 7.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"
          },
          "range": "<3.1.4"
        },
        {
          "source": 1113552,
          "name": "minimatch",
          "dependency": "minimatch",
          "title": "minimatch ReDoS: nested *() extglobs generate catastrophically backtracking regular expressions",
          "url": "https://github.com/advisories/GHSA-23c5-xmqv-rm74",
          "severity": "high",
          "cwe": [
            "CWE-1333"
          ],
          "cvss": {
            "score": 7.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"
          },
          "range": ">=9.0.0 <9.0.7"
        }
      ],
      "effects": [
        "editorconfig"
      ],
      "range": "<=3.1.3 || 9.0.0 - 9.0.6",
      "nodes": [
        "node_modules/@eslint/config-array/node_modules/minimatch",
        "node_modules/@eslint/eslintrc/node_modules/minimatch",
        "node_modules/editorconfig/node_modules/minimatch",
        "node_modules/eslint/node_modules/minimatch",
        "node_modules/minimatch"
      ],
      "fixAvailable": true
    },
    "picomatch": {
      "name": "picomatch",
      "severity": "high",
      "isDirect": false,
      "via": [
        {
          "source": 1115549,
          "name": "picomatch",
          "dependency": "picomatch",
          "title": "Picomatch: Method Injection in POSIX Character Classes causes incorrect Glob Matching",
          "url": "https://github.com/advisories/GHSA-3v7f-55p6-f55p",
          "severity": "moderate",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 5.3,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:L/A:N"
          },
          "range": "<2.3.2"
        },
        {
          "source": 1115551,
          "name": "picomatch",
          "dependency": "picomatch",
          "title": "Picomatch: Method Injection in POSIX Character Classes causes incorrect Glob Matching",
          "url": "https://github.com/advisories/GHSA-3v7f-55p6-f55p",
          "severity": "moderate",
          "cwe": [
            "CWE-1321"
          ],
          "cvss": {
            "score": 5.3,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:L/A:N"
          },
          "range": ">=4.0.0 <4.0.4"
        },
        {
          "source": 1115552,
          "name": "picomatch",
          "dependency": "picomatch",
          "title": "Picomatch has a ReDoS vulnerability via extglob quantifiers",
          "url": "https://github.com/advisories/GHSA-c2c7-rcm5-vvqj",
          "severity": "high",
          "cwe": [
            "CWE-1333"
          ],
          "cvss": {
            "score": 7.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"
          },
          "range": "<2.3.2"
        },
        {
          "source": 1115554,
          "name": "picomatch",
          "dependency": "picomatch",
          "title": "Picomatch has a ReDoS vulnerability via extglob quantifiers",
          "url": "https://github.com/advisories/GHSA-c2c7-rcm5-vvqj",
          "severity": "high",
          "cwe": [
            "CWE-1333"
          ],
          "cvss": {
            "score": 7.5,
            "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"
          },
          "range": ">=4.0.0 <4.0.4"
        }
      ],
      "effects": [],
      "range": "<=2.3.1 || 4.0.0 - 4.0.3",
      "nodes": [
        "node_modules/@rollup/pluginutils/node_modules/picomatch",
        "node_modules/npm-run-all2/node_modules/picomatch",
        "node_modules/picomatch",
        "node_modules/rollup-plugin-visualizer/node_modules/picomatch",
        "node_modules/unimport/node_modules/picomatch",
        "node_modules/unplugin-auto-import/node_modules/picomatch",
        "node_modules/unplugin-utils/node_modules/picomatch",
        "node_modules/unplugin/node_modules/picomatch",
        "node_modules/vitest/node_modules/picomatch"
      ],
      "fixAvailable": true
    },
    "rollup": {
      "name": "rollup",
      "severity": "high",
      "isDirect": false,
      "via": [
        {
          "source": 1113515,
          "name": "rollup",
          "dependency": "rollup",
          "title": "Rollup 4 has Arbitrary File Write via Path Traversal",
          "url": "https://github.com/advisories/GHSA-mw96-cpmx-2vgc",
          "severity": "high",
          "cwe": [
            "CWE-22"
          ],
          "cvss": {
            "score": 0,
            "vectorString": null
          },
          "range": ">=4.0.0 <4.59.0"
        }
      ],
      "effects": [],
      "range": "4.0.0 - 4.58.0",
      "nodes": [
        "node_modules/rollup"
      ],
      "fixAvailable": true
    }
  },
  "metadata": {
    "vulnerabilities": {
      "info": 0,
      "low": 0,
      "moderate": 3,
      "high": 10,
      "critical": 0,
      "total": 13
    },
    "dependencies": {
      "prod": 128,
      "dev": 545,
      "optional": 64,
      "peer": 0,
      "peerOptional": 0,
      "total": 672
    }
  }
}

npm warn Unknown env config "node-linker". This will stop working in the next major version of npm.
npm warn Unknown env config "store-dir". This will stop working in the next major version of npm.
npm warn Unknown env config "verify-deps-before-run". This will stop working in the next major version of npm.
npm warn Unknown env config "_jsr-registry". This will stop working in the next major version of npm.
npm warn Unknown project config "node-linker". This will stop working in the next major version of npm.
```
