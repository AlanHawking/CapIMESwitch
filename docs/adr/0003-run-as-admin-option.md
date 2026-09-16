# ADR 0003:选项面板新增「以管理员身份启动」

- 状态:已接受(2026-09)
- 关联决策:ADR 0002(8 语言)落地后新增面板选项;解决 UIPI 导致管理员窗口内
  CapsLock 失效的问题(见 `docs/glossary.md` 的 UIPI 词条)

## 背景

`WH_KEYBOARD_LL` 钩子与 `SendInput` 注入受 UIPI 限制:非提权实例无法感知/注入
管理员窗口(如管理员 cmd/pwsh)的输入。让整个程序以管理员运行即可覆盖两类窗口。
方案 B(计划任务)免开机 UAC 弹窗,是最优实现;本 ADR 将其做成面板选项,
勾选后:创建开机启动计划任务 + 提权重启生效;取消勾选:删除任务 + 降权重启。

## 已确认决策(2026-09 访谈定稿)

1. **联动覆盖(Q1,选 A)**:「以管理员身份启动」是开机启动的提权形态。
   勾选它 → 开机启动视为开启,Run 键清除、计划任务创建;
   取消它 → 回退 Run 键机制(若用户仍要开机启动)。面板两框联动,杜绝双实现。

2. **时序反转 + UAC 取消回滚(Q2,选 A)**:非提权进程无法创建 HIGHEST 任务,
   故实际流程为 保存(`run_as_admin=true`)→ UAC 提权重启 → **提权实例启动时**
   创建/校验任务(幂等)。UAC 取消 → 配置回写 `false`、勾选框复原,弹提示
   「已取消提权,设置未生效」。

3. **已提权状态行为(Q3)**:勾选保存 → 跳过 UAC、不重启,静默确保任务存在;
   取消保存 → 删除任务 → 写配置 `false` → 降权重启。

4. **降权重启失败降级(Q4,选 A)**:任务已删、配置已写 `false`,当前会话保持
   管理员继续运行,弹提示「无法自动降权,请退出后以普通方式重新启动」。

5. **托盘菜单一致性(Q5,选 A)**:管理员模式开启时,托盘「开机启动」菜单项
   置灰禁用,提示用户到选项面板管理。

6. **非管理员账户失败处理(Q6,选 A)**:提权实例启动时任务创建失败 →
   弹一次非致命错误框「开机自动提权启动设置失败,本会话仍以管理员运行」,继续。

7. **范围同步(Q7)**:setup.iss 卸载删任务、smoke.ps1 适配、README/术语表/ADR
   更新,版本号 bump 至 0.5.0。

8. **帮助文案(Q8,已简化)**:只说明用途,不描述内部动作;中文/英文定稿如下,
   其余 6 语言按同结构翻译。
   - 标签:`以管理员身份启动` / `Run as administrator`
   - 说明(中文):`以管理员身份开机自启,让 CapsLock 在管理员窗口中也能切换输入法`
   - 说明(英文):`Auto-start at login as administrator, so CapsLock keeps working in elevated windows too`

## 技术实现要点(事实约束)

1. **任务实现走 `schtasks.exe`**(零新依赖,符合"依赖极简"):
   任务名 `CapIMESwitchElevated`,`/SC ONLOGON /RL HIGHEST /F`。
   创建/删除/查询各一个辅助函数,经 `CreateProcessW` 调用并等待退出码。
2. **提权重启**:`ShellExecuteW(NULL, "runas", exe, "--restart", ...)`;
   降权重启:**经任务计划程序中转**——提权实例创建 `/RL LIMITED` 的一次性任务
   (`CapIMESwitchDeElevateTemp`)/Create + `/Run` 触发,任务计划程序服务以当前用户
   普通令牌启动 `--restart` 实例,新实例启动时按固定任务名清理。
   原令牌方案(`CreateProcessWithTokenW`/`CreateProcessAsUserW`)在 UAC 受限令牌环境下
   分别因 ERROR_ACCESS_DENIED(5) 与 ERROR_PRIVILEGE_NOT_HELD(1314) 失败
   (提权令牌缺 `SeAssignPrimaryTokenPrivilege`,实测 GetLastError=1300),已弃用。
3. **互斥体竞态**:新实例带 `--restart` 标志,启动时对
   `CapIMESwitch_SingleInstance` 做有限次重试(15s)再进入常规单实例检查。
4. **配置持久化**:`config.toml` 新增 `run_as_admin: bool`(默认 false,
   `#[serde(default)]` 兼容旧配置);面板初始勾选以配置为准。
5. **提权检测**:`IsUserAnAdmin()` 判定当前是否已提权,决定后续动作。

## 影响

- `src/config.rs`:`Config` 新增 `run_as_admin` 字段 + 测试
- `src/main.rs`:面板新勾选框与帮助、`save_options` 分支、提权/降权重启、
  schtasks 辅助、启动时任务协调、托盘菜单禁用、单实例重试
- `src/i18n.rs`:8 语言表新增 `label_run_as_admin` / `help_run_as_admin`
- `installer/setup.iss`:卸载时删除计划任务(`[Code]` 段)
- `smoke.ps1` / `README.md` / `docs/glossary.md`:同步

## 验证

- `cargo test` 全绿(含配置字段、8 语言文案非空、互斥体重试纯逻辑)
- `cargo build --release` 成功,无警告
- 手工:普通权限勾选保存 → UAC → 提权实例运行,`schtasks /Query` 见任务;
  管理员 cmd 内 CapsLock 生效;取消勾选 → 任务删除 + 降权重启
- 安装/卸载后任务清理验证
