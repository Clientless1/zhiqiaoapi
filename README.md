<div align="center">

# 智桥 API · zhiqiaoapi

<img src="frontend/public/logos/zhiqiao-original.png" alt="智桥 API" width="200" />

**开源多模型 API 网关与运营控制台**

统一接入文本、图片、视频和音频模型，提供模型广场、透明售价、上游渠道管理、采购成本比较、用量统计和计费运营。

<p align="center">
  <strong>中文</strong> |
  <a href="README.en.md">English</a>
</p>

[![License](https://img.shields.io/badge/license-AGPL--3.0-brightgreen.svg)](LICENSE)
[![Release](https://img.shields.io/github/v/release/Clientless1/zhiqiaoapi?color=brightgreen&include_prereleases)](https://github.com/Clientless1/zhiqiaoapi/releases/latest)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](backend/Cargo.toml)
[![Docker](https://img.shields.io/badge/docker-compose-blue.svg)](docker-compose.yml)

<p align="center">
  <a href="#-功能概览">功能概览</a> •
  <a href="#-为什么选择-zhiqiaoapi">为什么选择 zhiqiaoapi</a> •
  <a href="#-快速开始">快速开始</a> •
  <a href="docs/quickstart-10-minutes.zh.md">10 分钟部署</a> •
  <a href="docs/README.md">文档</a> •
  <a href="#-生产建议">生产建议</a> •
  <a href="#-获取帮助">获取帮助</a> •
  <a href="docs/deployment/cluster.zh.md">集群部署</a>
</p>

</div>

---

## 📝 项目介绍

智桥 API 是基于 **Rust + Vue 3 + PostgreSQL** 构建的开源大模型 API 中转与管理平台。它将多个上游服务统一到兼容 OpenAI / Anthropic 的接口下，适用于团队自用、企业私有化部署和面向开发者的 API 服务运营。

用户可以在模型广场查看真实发布的模型、能力、计费单位和价格，再创建站内 API Key 接入客户端。管理员可以集中管理上游协议、地址、凭证、模型售价和采购报价，比较不同渠道的预计成本，并通过独立的用户控制台和管理控制台查看用量、账单与运营数据。

仓库地址：[Clientless1/zhiqiaoapi](https://github.com/Clientless1/zhiqiaoapi)

首页、登录页及控制台采用统一的简约界面，提供首页控制台入口、侧栏模型广场、站点品牌配置和新的蓝色桥形浏览器图标。

> [!IMPORTANT]
> zhiqiaoapi 适用于合法、授权的 AI API 网关、企业级鉴权、多模型管理、用量统计、成本归集和私有化部署场景。使用者应合法获取上游 API key、账号、模型服务和接口权限，并遵守上游服务条款及所在地法律法规。

---

## 🔎 Search Keywords

`LLM API gateway` · `AI gateway` · `OpenAI-compatible proxy` · `Anthropic-compatible API` · `realtime ASR` · `OpenAI video API` · `Alibaba Cloud Bailian` · `self-hosted AI infrastructure` · `smart model routing` · `AI app management` · `WeCom integration` · `Feishu integration` · `DingTalk integration` · `webhook AI app` · `web chat widget` · `multi-tenant API keys` · `usage tracking` · `cost management` · `billing` · `Rust`

---

## ✨ 功能概览

模型发布、报价口径和采购功能边界见：[模型广场与上游供应链](docs/model-plaza-supply-chain.zh.md)。

<table>
  <thead>
    <tr>
      <th width="180">能力</th>
      <th>说明</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>🗂️ 模型广场</td>
      <td>公开展示已发布模型，支持类型与供应商筛选、搜索、价格排序、渠道价格方案、缓存价格、视频档位、请求示例和费用预估；登录后应用用户组价格倍率。</td>
    </tr>
    <tr>
      <td>📦 上游供应链</td>
      <td>按渠道与模型维护供应商、采购报价、月度预算和备注，以假设用量比较成本与基础毛利，辅助配置渠道优先级和权重。</td>
    </tr>
    <tr>
      <td>🏢 企业统一入口</td>
      <td>提供企业自己的大模型 API 入口，兼容 OpenAI 和 Anthropic 接口，并集中托管上游凭证和访问策略。</td>
    </tr>
    <tr>
      <td>🧩 项目化管理</td>
      <td>以项目承载业务、团队或成本单元，管理成员、模型权限、预算额度和成本归集。</td>
    </tr>
    <tr>
      <td>🔑 独立 API Key</td>
      <td>为团队、客户或内部应用分发独立 API Key，隔离调用权限、额度和用量追踪。</td>
    </tr>
    <tr>
      <td>🧰 应用管理</td>
      <td>创建企业微信、飞书、钉钉、Webhook 和网页组件应用，让员工或外部系统直接与大模型对话。</td>
    </tr>
    <tr>
      <td>🧠 智能模型路由</td>
      <td>识别多模态、工具调用、代码、推理强度和上下文长度等请求特征，按复杂度选择不同模型，并保留可追踪的路由决策。</td>
    </tr>
    <tr>
      <td>🧭 可靠上游路由</td>
      <td>按模型、优先级和权重分配请求，并在上游 key 异常时自动冷却和切换。</td>
    </tr>
    <tr>
      <td>🎙️ 语音/视频处理</td>
      <td>提供 OpenAI 兼容的文件转写、Realtime 实时语音识别和视频生成接口，统一处理异步任务、结果查询、故障切换，以及按音频时长或视频规格计费。</td>
    </tr>
    <tr>
      <td>📊 用量与成本分析</td>
      <td>记录用户、项目、API Key、模型和通道维度的调用明细，并汇总成本、请求数、成功率与 Token 用量，支持时间筛选、多级下钻和 CSV 导出。</td>
    </tr>
    <tr>
      <td>💳 服务计费</td>
      <td>支持内部模式和计费模式，以及项目、API Key、模型三级额度、预留结算、充值支付和可追踪账本。</td>
    </tr>
    <tr>
      <td>🛍️ 用户运营</td>
      <td>提供模型套餐、优惠券、兑换码、订单、资金流水与站内通知，管理员可在用户运营页面维护相关配置。</td>
    </tr>
    <tr>
      <td>🔌 客户端接入</td>
      <td>提供站内 API Key 管理、Claude Code / Codex / OpenCode 配置导出和 CCSwitch 导入链接，支持密钥模型范围配置。</td>
    </tr>
    <tr>
      <td>🚀 集群部署</td>
      <td>支持单机 Compose 部署，也支持基于外部 PostgreSQL 和 Redis 的生产多副本部署。</td>
    </tr>
  </tbody>
</table>

---

## 🧭 页面与功能入口

| 入口                                                  | 功能                                     |
| ----------------------------------------------------- | ---------------------------------------- |
| `/`                                                   | 首页、模型搜索和控制台入口               |
| `/models`                                             | 公开模型广场和基础售价                   |
| `/home/models`                                        | 登录用户的模型广场和用户组价格           |
| `/home/overview`                                      | 余额、用量与常用操作                     |
| `/home/apikeys`                                       | 站内 API Key、客户端配置与 CCSwitch 导入 |
| `/home/packages`、`/home/coupons`、`/home/redeem`     | 套餐、优惠券与兑换                       |
| `/home/orders`、`/home/ledger`、`/home/notifications` | 订单、资金流水和通知                     |
| `/admin/channels`                                     | 上游渠道、模型发现、售价和诊断           |
| `/admin/supply-chain`                                 | 采购报价、供应商与预算场景比较           |
| `/admin/user-operations`                              | 套餐、优惠券、兑换码和通知管理           |

管理入口需要管理员权限。采购报价仅管理员可见；公开模型价格与某个 API Key 实际可调用的模型范围是两套规则。采购场景比较不会自动选择最低报价渠道，月度预算也不会自动停止渠道。

## 🖼️ 界面预览

以下为已有管理功能截图；当前源码中的首页、模型广场与控制台已更新为统一样式。

<p align="center">
  <img src="docs/assets/admin-usage-statistics.png" alt="zhiqiaoapi 消费统计截图" width="920">
</p>

<p align="center">
  <img src="docs/assets/admin-usage-records.png" alt="zhiqiaoapi 使用明细截图" width="920">
</p>

<p align="center">
  <img src="docs/assets/admin-channel-diagnostics.png" alt="zhiqiaoapi 通道诊断截图" width="920">
</p>

---

## 🧠 为什么选择 zhiqiaoapi

- **接入更省心**：现有客户端和内部服务少改配置，就能统一接入大模型能力，把试用和上线周期都缩短。
- **密钥更安全**：上游凭证不用散落在各个应用里，权限、额度和访问策略集中管理，团队用起来更安心。
- **成本更清楚**：谁在用、哪个项目在用、用了多少 Token 和费用，都能按维度看清楚，核算和排查更顺手。
- **团队更好协作**：不同团队、客户或内部应用可以用项目隔开，边界清楚，统计清楚，协作也更轻松。
- **AI 更容易落地**：通过企业微信、飞书、钉钉、Webhook 和网页组件，把大模型能力放到员工和系统每天都会用到的入口里。
- **场景覆盖更完整**：既能做企业内部 AI 网关，也能延伸到额度、充值、支付和计费运营，少做很多重复建设。

---

## 🧭 服务模式

zhiqiaoapi 首次运行时需要选择内部模式或计费模式。内部模式适合企业自用和团队协作，计费模式适合面向客户或开发者提供服务。两种模式都保留统一入口、凭证托管、模型路由和用量记录，主要区别在于是否要求可用额度，以及是否接入支付通道。

<table>
  <thead>
    <tr>
      <th width="150">模式</th>
      <th>适用场景</th>
      <th width="180">调用限制</th>
      <th>配置重点</th>
    </tr>
  </thead>
  <tbody>
    <tr>
      <td>🏠 内部模式</td>
      <td>公司、部门或项目组自用，给内部应用、自动化脚本和成员分发 API Key。</td>
      <td>默认不要求可用额度即可调用。</td>
      <td>仍会记录用量和费用，便于成本分析和内部核算。</td>
    </tr>
    <tr>
      <td>💰 计费模式</td>
      <td>面向客户、开发者或外部用户提供收费的大模型调用服务。</td>
      <td>用户需要有可用额度后才能调用。</td>
      <td>上线前需要配置模型价格、充值套餐和支付通道。</td>
    </tr>
  </tbody>
</table>

---

## 🚀 快速开始

如果你是第一次试用，建议先按这篇教程走完整闭环：[10 分钟部署 zhiqiaoapi](docs/quickstart-10-minutes.zh.md)。

### 🐳 Docker 安装

Docker 安装是最快的部署方式，不需要在宿主机单独安装 Rust、Node.js 或 pnpm。需要多副本和横向扩展的生产环境，请查看：[集群部署文档](docs/deployment/cluster.zh.md)。

#### 从源码部署当前版本

本仓库的最新界面和功能以当前源码为准。首次安装可使用 Docker Compose 构建，无需在宿主机安装 Rust 或 Node.js：

```bash
git clone https://github.com/Clientless1/zhiqiaoapi.git
cd zhiqiaoapi

# 仅在新安装时生成密码，保留已有部署的 .env
[ -f .env ] || (umask 077; printf 'POSTGRES_PASSWORD=%s\n' "$(openssl rand -hex 24)" > .env)
docker compose -f docker-compose.build.yml up -d --build
```

访问 `http://localhost:8080` 完成初始化，然后按以下顺序配置：

1. 创建管理员并选择内部模式或计费模式。
2. 在渠道管理添加上游协议、地址和凭证，发现模型并运行诊断。
3. 启用模型与对应售价；满足发布条件后，模型广场会显示这些模型。
4. 按需维护采购报价、用户组价格、充值或套餐配置。
5. 在用户控制台创建站内 API Key，使用下方接入说明测试调用。

#### 使用预构建镜像

单机 Compose 会同时启动前端 Nginx、后端和 PostgreSQL。预构建镜像需要对应的 GitHub Actions 构建成功并具备拉取权限；需要立即部署本次源码时使用上面的构建命令。

首次启动前，在 `.env` 中生成数据库密码；升级已有站点时保留原 `.env` 和原密码：

```bash
# 仅用于新安装，不会覆盖已有 .env
[ -f .env ] || (umask 077; printf 'POSTGRES_PASSWORD=%s\n' "$(openssl rand -hex 24)" > .env)
```

```bash
# 使用预构建镜像，无需在服务器上编译
docker compose up -d
```

如果希望从源码本地构建镜像，或需要使用中国大陆镜像源构建，可以使用：

```bash
# 海外（Docker Hub 可直接访问）
docker compose -f docker-compose.build.yml up -d --build

# 中国大陆（使用国内镜像源）
docker compose -f docker-compose.cn.yml up -d --build
```

启动后访问 `http://服务器IP:8080`，首次运行向导会引导你完成管理员、服务模式、初始上游、价格、SMTP 和支付等配置。

#### 绑定域名和宿主机 Nginx

使用 Docker Compose 安装时，Compose 默认会把服务暴露到宿主机 `8080` 端口。宿主机 Nginx 可直接反向代理到 `http://127.0.0.1:8080`：

```bash
sudo cp deploy/nginx/docker-compose.conf.example /etc/nginx/conf.d/zhiqiaoapi.conf
sudo vim /etc/nginx/conf.d/zhiqiaoapi.conf
sudo nginx -t
sudo systemctl reload nginx
```

#### 检查运行状态

部署完成后，可以先查看容器是否都处于 `running` 或 `healthy` 状态：

```bash
docker compose ps
```

通常会看到 `postgres`、`backend`、`web` 三个服务。若服务状态不是 `running`/`healthy`，可以查看日志定位原因：

```bash
docker compose logs -f
```

也可以只查看某个服务的日志，例如单机部署的后端：

```bash
docker compose logs -f backend
```

最后在浏览器访问 `http://服务器IP:8080`。如果可以打开首次运行向导或登录页面，通常表示前端、后端和反向代理链路已经正常。

#### 管理员密码恢复

如果管理员忘记密码或账号被锁定，可以在后端容器中重置：

```bash
docker compose exec backend zhiqiaoapi admin reset-password --username admin
```

如果管理员用户名不是 `admin`，请替换 `--username`。

### 🧑‍💻 源码本地运行

源码运行适合二次开发、调试和自定义部署。请先准备 PostgreSQL 16、Rust 1.94+、Node.js 20+ 和 pnpm。

#### 准备数据库

进入 PostgreSQL：

```bash
sudo -u postgres psql
```

```sql
CREATE USER zhiqiaoapi WITH PASSWORD 'change-me';
CREATE DATABASE zhiqiaoapi OWNER zhiqiaoapi;
\q
```

数据库连接地址：

```text
postgres://zhiqiaoapi:change-me@localhost:5432/zhiqiaoapi
```

#### 开发运行

分别启动后端、定时任务和前端：

```bash
cargo run -p zhiqiaoapi
```

```bash
cargo run -p zhiqiaoapi-scheduler
```

```bash
cd frontend
pnpm install
pnpm dev --host 0.0.0.0
```

打开 `http://localhost:5173`；远程开发时使用 `http://服务器IP:5173`。按照首次运行向导完成数据库连接、管理员账号、服务模式和初始上游配置。保存运行配置后如提示重启，重新启动后端即可。

#### 正式运行

构建后端、定时任务和前端：

```bash
cargo build --release -p zhiqiaoapi -p zhiqiaoapi-scheduler
cd frontend
pnpm install
pnpm build
cd ..
```

分别启动后端和定时任务：

```bash
BIND_ADDR=127.0.0.1:8080 ./target/release/zhiqiaoapi
```

```bash
./target/release/zhiqiaoapi-scheduler
```

正式环境应使用 systemd、supervisord 等工具保持进程常驻，并通过 Nginx 托管 `frontend/dist`、代理后端接口和配置 HTTPS。生产环境仍建议优先使用 Docker Compose 或集群部署方案。

---

## 🔌 API 与客户端接入

Docker 默认 API Base URL 为 `http://127.0.0.1:8080/v1`。Vite 默认开发地址为 `http://127.0.0.1:5173`，端口占用时以终端实际输出为准；例如本地启动在 `5175` 时，可使用 `http://127.0.0.1:5175/v1`，其 API 请求由开发代理转发到后端。

**客户端填写用户控制台创建的站内 API Key。上游 Key 填在管理员渠道配置中。** 复制地址时确保 `/v1` 只出现一次，所选模型应在渠道启用且在该站内 Key 的授权范围内。

```bash
export ZHIQIAO_BASE_URL="http://127.0.0.1:8080/v1"
export ZHIQIAO_API_KEY="YOUR_SITE_API_KEY"

curl "$ZHIQIAO_BASE_URL/models" \
  -H "Authorization: Bearer $ZHIQIAO_API_KEY"

curl "$ZHIQIAO_BASE_URL/responses" \
  -H "Authorization: Bearer $ZHIQIAO_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"YOUR_ENABLED_MODEL","input":"你好"}'
```

| 能力               | 接口示例                                                                 |
| ------------------ | ------------------------------------------------------------------------ |
| 模型列表           | `GET /v1/models`                                                         |
| 文本与推理         | `POST /v1/chat/completions`、`POST /v1/responses`                        |
| Anthropic Messages | `POST /v1/messages`                                                      |
| 图片               | `POST /v1/images/generations`、`POST /v1/images/edits`                   |
| 视频               | `POST /v1/videos`、任务查询和内容获取                                    |
| 音频               | `POST /v1/audio/transcriptions`、`POST /v1/audio/speech`、`/v1/realtime` |
| 嵌入与审核         | `POST /v1/embeddings`、`POST /v1/moderations`                            |

具体能力取决于上游协议、模型与权限。`/v1/models` 返回成功只说明模型列表接口可访问，仍需验证实际调用；部分第三方上游只支持 Responses，需使用兼容的客户端请求方式。其他设备访问时将 `127.0.0.1` 换成服务主机的可达地址。

在 API Key 页面可导出客户端配置或通过 CCSwitch 导入。Codex 使用 Responses 协议；导出内容包含该站内 Key，请保存在本机，不要提交到代码仓库。

## 🏗️ 项目结构与开发检查

```text
backend/                Rust API、鉴权、转发、计费与数据库迁移
scheduler/              通道健康检查、模型同步与定时任务
crates/upstream-probe/  后端与调度器共用的上游协议探测
frontend/               Vue 3 + TypeScript 用户端与管理端
docs/                   部署、设计与功能说明
deploy/                 Nginx 和部署示例
```

```bash
# 仓库根目录
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 前端
cd frontend
pnpm install --frozen-lockfile
pnpm lint
pnpm test
pnpm build
```

完整开发说明见 [Development Guide](docs/development.md)。默认测试不调用真实上游；依赖 PostgreSQL 的集成测试和真实服务冒烟测试需按各自文档单独执行。

---

## ✅ 生产建议

上线前至少确认：

- **安全与域名**：使用强管理员密码和随机生成的 `ADMIN_TOKEN_SECRET`、`UPSTREAM_SECRET_KEY`；通过 HTTPS 对外提供服务，并正确配置 `PUBLIC_BASE_URL` 和反向代理。
- **上游可用性**：通过通道诊断验证模型、端点和凭证，保持定时任务进程运行，以执行通道探测和模型目录同步。
- **计费配置**：计费模式上线前检查模型价格、额度策略、充值套餐和支付回调；内部模式也应确认用量与成本记录正常。
- **数据持久化**：持久化并定期备份 PostgreSQL、运行配置和系统密钥；使用后台图片生成或其他需要本地响应资产的任务时，同时持久化 `ZHIQIAOAPI_ASSET_DIR`。
- **集群部署**：多副本部署需要共享 PostgreSQL、Redis 和系统密钥，并根据实际负载规划 API、Worker 与 Scheduler 角色。

更多配置和容量限制请参考：

- [单机部署](docs/deployment/standalone.zh.md)
- [集群部署](docs/deployment/cluster.zh.md)

---

## 📄 开源协议

zhiqiaoapi 社区版使用 [GNU Affero General Public License v3.0](LICENSE) 开源协议（`AGPL-3.0-only`）。

zhiqiaoapi 同时提供分层商业授权：普通企业内部使用可申请免费的书面 Internal Commercial License；客户交付、托管服务、SaaS、OEM、MSP、白标、转售等场景需要单独 Commercial License；企业版功能使用商业 EULA。详见 [LICENSING.md](LICENSING.md)。

zhiqiaoapi 名称、Logo 及相关标识不随 AGPL 授权，使用边界见 [TRADEMARKS.md](TRADEMARKS.md)。

---

## 🙋 获取帮助

| 类型          | 链接                                                              |
| ------------- | ----------------------------------------------------------------- |
| 🐛 问题反馈   | [GitHub Issues](https://github.com/Clientless1/zhiqiaoapi/issues) |
| 🤝 代码贡献   | [Pull Requests](https://github.com/Clientless1/zhiqiaoapi/pulls)  |
| 💬 官方 QQ 群 | 群号：`1179649618`                                                |

<table>
  <tr>
    <th>微信联系</th>
    <th>官方 QQ 群</th>
  </tr>
  <tr>
    <td><img src="docs/assets/wechat.png" alt="zhiqiaoapi 微信联系二维码" width="220" /></td>
    <td><img src="docs/assets/qq.png" alt="zhiqiaoapi 官方 QQ 群二维码" width="220" /></td>
  </tr>
</table>

### 升级与计费可靠性

Compose 使用 `backend-assets` 持久化生成图片和排队请求文件。旧部署升级前，先将旧后端容器中的 `/tmp/zhiqiaoapi/assets` 复制到该卷，再重建容器。备份须包含数据库、配置和资源卷。分布式 API 与 worker 在不同主机时，需将共享存储挂到同一 `ZHIQIAOAPI_ASSET_DIR`；普通 Docker 卷只在同一主机共享。

每日用量和仪表盘统一按 UTC 统计，请求时间可按本地时区显示。修改密码会撤销旧用户登录和找回密码链接；本次升级后用户需重新登录。套餐停售与停服分开，停售保留已有订阅；存在有效付费订阅时拒绝停服。历史迁移保持不变，启动时仅兼容三个已知的任务品牌迁移校验值，由新迁移 14 统一任务记录；未知迁移改动仍会阻止启动。

已发出的请求即使在余额预留回收后才完成，也会保留应付用量。存在未结清用量时，同一用户更换密钥或项目不能绕过余额检查。个人密钥及其模型钱包的欠款先从原账户结算，不足时可从同用户默认项目的可用余额补扣；项目钱包的欠款仅从原项目钱包结算。项目转移时，项目钱包的归属随项目更新，个人密钥钱包仍归原用户。资金流水记录实际扣款账户，自动结算按后台重试周期进行，预留中的额度不会用于补扣。
