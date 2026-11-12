# 当前线上版本与部署

本次同步包含截至 2026-11-12 的线上源码：官网与控制台界面、品牌图标、模型广场、API 调试台、CC Switch 导入、优惠券发放、第三方登录、数据卷保护及备份恢复脚本。实际运行配置、数据库、渠道凭证、用户 API Key、备份和静态构建产物不进入 Git。

京东云当前域名入口继续使用 Cloudflare Tunnel，经主机 Nginx `127.0.0.1:8091` 转发到前端 `127.0.0.1:8090`。静态资源预压缩、缓存、并行页面加载及性能验证见 [隧道优化说明](ops/tunnel-performance.md)。

## 从源码部署

需要 Docker Compose。先在项目根目录创建 `.env`，设置新的数据库密码，首次初始化时再配置站点和管理员：

```dotenv
POSTGRES_PASSWORD=replace-with-a-long-random-password
WEB_PORT=8090
```

三个数据卷使用固定名称的 external 卷。首次安装时创建它们；已有部署继续复用原卷，不要删除或替换现有卷：

```sh
docker volume create zhiqiaoapi_postgres-data
docker volume create zhiqiaoapi_backend-config
docker volume create zhiqiaoapi_backend-assets
docker compose -f docker-compose.cn.yml up -d --build --wait
```

海外网络可使用 `docker-compose.build.yml`。其中保留了服务器使用的 `host.docker.internal:7890` 上游代理配置，使用前根据实际网络调整 `HTTP_PROXY`、`HTTPS_PROXY`。

启动后访问 `http://服务器地址:8090`。`/readyz` 应返回 `ok: true`。域名反向代理或隧道应指向同机服务 `http://127.0.0.1:8090`；隧道凭证不在仓库内。

## 只更新前端

常规源码构建可直接运行：

```sh
docker compose -f docker-compose.cn.yml up -d --build --no-deps web
```

线上使用的 branding 覆盖文件支持在构建机预编译前端，然后仅打包 nginx 镜像。使用 Node.js 20+ 和 pnpm 10.25.0，在项目根目录执行：

```sh
pnpm --dir frontend install --frozen-lockfile
pnpm --dir frontend build
mkdir -p deploy/branding/web
cp -R frontend/dist/. deploy/branding/web/
docker compose -f docker-compose.cn.yml -f docker-compose.branding.yml up -d --build --no-deps web
```

`deploy/branding/web` 是构建输出，由 `.gitignore` 排除。生产环境不要设置 `VITE_DESIGN_PREVIEW=true`。

## 客户端与调试台

- `/home/apikeys` 中每个启用的 Key 支持导入 CC Switch，选择 Codex、Claude Code 或 OpenCode，以及该 Key 允许的模型。也可复制客户端配置。
- GPT/Codex 模型的导入配置默认使用 Responses；OpenCode 也按模型选择对应协议。
- 调试台支持 Responses 和 Chat Completions，自动模式为 GPT/Codex 选择 `/v1/responses`。上游只支持 Responses 时，外部客户端也应使用该接口。

## 数据持久化与备份

数据库、运行配置（含加密密钥）、资源分别保存在上述三个固定数据卷中。数据库与配置密钥必须配套备份、恢复。GitHub 源码不能替代业务数据备份。

备份与恢复演练说明见 [ops/README.md](ops/README.md)。已有服务器的 systemd 任务文件保存在 `deploy/systemd/zhiqiaoapi-backup.service` 和 `.timer`；移到其他服务器时先调整用户名、项目目录及备份目录，再安装和启用。
