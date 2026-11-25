# VP429

面向 **AI API 高频调用与防 429 限流** 场景设计的轻量级 Windows 桌面代理客户端，基于 **Rust + GPUI + Mihomo (Clash Meta)** 构建。

> 核心能力：对指定监听域名（如 `aiplatform.googleapis.com`、`generativelanguage.googleapis.com`）执行**选择性 HTTPS MITM**，在底层毫秒级拦截 `HTTP 429 Too Many Requests`，自动将限流节点置入冷却、轮换至下一个健康节点并**透明重放原请求**，调用方全程无感知。

---

## 核心特性

- **HTTP 429 自动拦截与透明重放**
  - 仅解密配置的目标监听域名，其余所有 HTTPS 流量直接透传不解密。
  - 捕获上游 `429` 响应后自动冷却当前节点、切换至下一个可用节点并重放请求（可配置冷却时长、最大重试次数及最大请求体缓冲）。
  - 支持 3 种节点轮询策略：**仅 429 切换** / **逐请求轮询** / **定时轮询**。
- **系统代理 & WinTUN 全局无回环架构**
  - 内置托管 `mihomo` 内核（首次运行可自动下载或指定本地路径），支持一键切换 Windows 系统代理与 TUN 虚拟网卡全局接管。
  - 采用专用 Bypass 出站端口，彻底杜绝 TUN 模式下的本地 MITM 回环死循环。
- **多订阅配置与节点管理**
  - 支持 Clash / Mihomo YAML 订阅链接导入、定时/手动刷新，自动解析流量配额、到期时间与配置名。
  - 内置真连接延迟测速、国旗地区识别、底部快捷上拉切换节点弹窗与一键「模拟 429」压测验证。
- **原生轻量桌面体验**
  - 采用 **Win11 Fluent × Braun** 工业风原生 GPUI 界面，无 Electron/WebView 运行时开销。
  - 支持最小化至 Windows 系统托盘并自动收缩内存工作集，内置事件驱动 60fps 微动效与加载动画。

---

## 工作原理

```text
Client (IDE / CLI / Browser)
   │
   ├─ 系统代理 :7890 ──────────────────────────┐
   │                                           ▼
   └─ WinTUN 全局 ──► Mihomo (规则分流) ──► VP429 本地代理 (:7890 / :17891)
                         │                     │
                         │ 非监听域名直出        │ 命中监听域名 -> 选择性 HTTPS MITM
                         ▼                     │  ├─ 正常响应 -> 透明返回给客户端
                    VERTEX-POOL                │  └─ 命中 429 -> 节点冷却 + 毫秒级切换 + 自动重放
                         ▲                     │
                         └── 无回环直出 (:17892) ┘
```

### 默认端口分配

| 端口 | 用途 |
| :--- | :--- |
| `7890` | 本地主代理监听端口（HTTP / HTTPS，供系统代理与 `HTTPS_PROXY` 使用） |
| `17890` | Mihomo 内部 Mixed 端口（非监听域名直通流量） |
| `17891` | TUN 模式回流 MITM 监听端口 |
| `17892` | Mihomo 无回环直出端口（直接绑定 `VERTEX-POOL`） |
| `19090` | Mihomo External Controller API（毫秒级切换节点与真连接测速） |

> 所有端口均可在「系统设置」页面自定义修改。

---

## 快速开始

### 1. 从源码构建

环境要求：Windows 10/11 + Rust 工具链（`cargo`）。

```powershell
git clone https://github.com/novtia/VP429.git
cd VP429
cargo build --release
```

编译产物位于 `target\release\VP429.exe`。

### 2. 使用步骤

1. 启动 `VP429.exe`，进入 **配置文件** 页面，点击右下角 `+` 添加 Clash / Mihomo YAML 订阅链接。
2. 进入 **429 策略** 页面，点击 **安装 CA 证书**（将本地自签名 `VP429 Local CA` 安装至当前用户 `CurrentUser\Root`，仅用于解密你指定的监听域名）。
3. 返回 **仪表盘**，点击 **▶ 启动代理** 即可开始工作。
   - 如需开启 **虚拟网卡 (TUN)** 全局接管，请以管理员身份运行程序。
   - 如需在局域网或 WSL2 中使用，可在 **系统设置** 中开启「允许局域网 / WSL 连接」。

### 3. 运行测试

```powershell
cargo test
```

包含配置解析、节点调度冷却、以及端到端 HTTPS MITM 拦截 429 并自动轮换重试的集成测试。

---

## 项目结构

```text
src/
├── main.rs            # 应用入口、窗口配置与生命周期管理
├── config.rs          # 配置持久化、多订阅结构与数据目录管理
├── engine.rs          # 核心代理引擎编排与遥测状态汇总
├── mihomo.rs          # Mihomo 内核进程托管、配置生成与 API 控制
├── scheduler.rs       # 节点池状态机、429 冷却与轮询调度器
├── subscription.rs    # Clash/Mihomo 订阅拉取、节点与流量配额解析
├── sysproxy.rs        # Windows 注册表系统代理开关
├── tray.rs            # Windows 系统托盘与工作集内存回收
├── state.rs           # 实时日志事件总线
├── mitm/
│   ├── ca.rs          # 本地自签名 CA 生成与 Windows 证书安装检测
│   └── proxy.rs       # 选择性 HTTPS MITM 代理与 429 透明重放引擎
└── ui/
    ├── mod.rs         # MainView 状态、导航路由与动画帧调度
    ├── shell.rs       # 窗口框架（无边框标题栏、侧边栏、自定义滚动条）
    ├── anim.rs        # 事件驱动 60fps 动画引擎与加载波浪组件
    ├── theme.rs       # Win11 Fluent × Braun 色板与样式 Token
    ├── icons.rs       # 内嵌矢量 SVG 图标
    ├── flags.rs       # 节点地区识别与几何国旗渲染
    ├── input.rs       # 自定义文本输入组件
    └── pages/         # 独立页面视图（dashboard / nodes / profiles / strategy / logs / settings）
```

---

## 注意事项

- 本地数据目录默认位于 `%LOCALAPPDATA%\vp429\VP429\data`（包含 `config.json`、本地生成的 CA 证书与 `bin\mihomo.exe`）。
- 本项目仅提供网络代理调度与本地开发调试工具，请遵守当地法律法规及服务提供商的使用条款。
