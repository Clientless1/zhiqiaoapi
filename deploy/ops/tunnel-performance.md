# Cloudflare Tunnel 加载优化

更新：2026-11-12（北京时间）。保留原有域名、DNS、隧道及 QUIC 连接。

## 当前路径

```text
用户 → Cloudflare → cloudflared → 主机 Nginx 127.0.0.1:8091
     → 前端 Nginx 127.0.0.1:8090 → Rust API → PostgreSQL
```

## 优化内容

- 构建时为 JS、CSS 和 SVG 生成 `.gz` 文件。带 Vite 内容指纹的 JS/CSS 由 Nginx `gzip_static` 提供，并返回 `public, max-age=31536000, immutable`。目前构建输出的 SVG 内联于 JS/CSS，公开文档截图不使用长期不可变缓存。
- HTML 保持 `no-store, no-cache`；API、OAuth、用户数据和模型请求不加入静态缓存规则。Cloudflare 已缓存的旧资源可能在原有四小时 TTL 到期后才采用新缓存头；无需刷新全部缓存。
- 当前页面的懒加载代码与安装状态、登录状态检查同时开始。导航权限和会话检查仍完整执行，未登录访问工作台仍跳转登录。
- 主机 Nginx 到前端使用连接池（每个 worker 最多 32 条空闲连接）；普通请求不发送 `Connection: close`。API 继续关闭响应/请求缓冲，保留 WebSocket 升级及 1200 秒读写超时。
- 本次部署保留前一版本的静态资源，避免已打开的页面在延迟加载时遇到 404。后续滚动发布也应保留仍可能被旧页面引用的资源。

## 验证和实际收益

- 前端 23 个测试文件、83 项测试通过；类型检查、Vite 构建和修改文件的 ESLint 检查通过。
- 已验证 gzip 解压结果与原文件逐字节一致；HTML 不存储、未授权 API 返回 401、未知资源返回 404、旧版本脚本仍可读取。
- 在隔离测试服务中穿过两层 Nginx，SSE 三段数据分别在约 0.002、0.302、0.602 秒到达，WebSocket 握手为 101。未消耗收费模型调用。
- 公网抽样中新版本入口脚本首次请求约 1.09 秒（CF MISS），重复请求约 0.38 秒（CF HIT）；其余新脚本缓存命中约 0.42～0.49 秒。该比较体现缓存命中收益，并非严格的全站改版前后性能对照。浏览器正常复访还可直接复用本地不可变缓存。
- 动态 `/readyz` 仍约 1.1 秒。源站同机约 2 毫秒，四条隧道连接落在 Los Angeles 节点；源站缓存优化不能消除这段公网链路延迟。
- 临时 HTTP/2 隧道实验没有测出明显收益，已撤销实验连接并保留原 QUIC；服务器缺少 IPv6 出站路由，没有强制切换 IPv6。没有修改 DNS、固定 Cloudflare 边缘 IP 或将业务流量接入 OAuth 专用代理。

## 配置与回退

仓库中的 `deploy/nginx/tunnel-origin.conf` 对应生产主机的 `/etc/nginx/sites-available/zhiqiao-tunnel`，需在 Nginx `http` 上下文载入。它依赖当前主机已有的安全 snippet 和 `zhiqiao_secure` 日志格式，不应脱离主机配置单独替换到其他机器。

生产前端源码构建仍使用 `frontend/Dockerfile`。本次镜像采用当前 Nginx 运行镜像，叠加通过类型检查的前端产物及预压缩文件；没有重建后端或数据库。

本次受限回退目录为 `/home/zhiqiao/build-tunnel-optimize-20261112/rollback`，旧镜像标签为 `zhiqiaoapi-web:before-tunnel-opt-20261112`。需要撤销本次变更时，在生产主机执行：

```sh
bash /home/zhiqiao/build-tunnel-optimize-20261112/rollback/restore.sh
```

该脚本恢复原 Nginx 配置及前端镜像，不操作数据库卷。验收通过后，临时自动回退定时器已经停止。

只读状态检查：

```sh
nginx -t
systemctl is-active cloudflared
curl -fsS http://127.0.0.1:20241/ready
curl -fsS -H 'Host: zhiqiaoapi.fun' http://127.0.0.1:8091/readyz
docker compose -f /home/zhiqiao/zhiqiaoapi/docker-compose.cloud.yml ps
```
