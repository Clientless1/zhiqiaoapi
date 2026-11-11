# OAuth 专用出站代理

现有服务器安装 Mihomo 官方发行版，仅用于 GitHub 和 Google OAuth。代理没有公开监听端口，没有启用 TUN 或系统级代理。业务代码只有 OAuth 专用 HTTP 客户端读取 `OAUTH_PROXY_URL`；模型路由、支付、邮件继续使用原有连接。

## 布局

- 二进制：`/usr/local/bin/zhiqiao-oauth-proxy`
- systemd：`zhiqiao-oauth-proxy.service`，开机自动启动
- 配置：`/etc/zhiqiao-oauth-proxy/config.json`，目录 `0700`、文件 `0600`
- Docker 内部网络：`zhiqiao-oauth-egress`，`172.30.49.0/29`
- 代理入口：`http://172.30.49.1:17897`，仅后端所在内部网络可访问
- 管理接口：`127.0.0.1:19090`，使用独立密钥
- 运行环境文件：backend-config 卷中的 `.env`，`OAUTH_PROXY_URL=http://172.30.49.1:17897`

规则仅放行 `github.com`、`api.github.com`、`oauth2.googleapis.com` 和 `openidconnect.googleapis.com`，其余目标拒绝。没有设置 `HTTP_PROXY`、`HTTPS_PROXY` 或 `ALL_PROXY`。

## 检查

```sh
systemctl is-active zhiqiao-oauth-proxy
systemctl show zhiqiao-oauth-proxy -p NRestarts -p ExecMainStatus
docker network inspect zhiqiao-oauth-egress
curl -fsS http://127.0.0.1:8090/readyz
curl -fsS http://127.0.0.1:8090/api/auth/providers
```

供应商接口需要凭证，直接探测 token 端点返回 400、用户端点返回 401 可以证明网络与 TLS 已连通，不能单独证明完整登录成功。完整验证还需要用户在供应商授权页确认并成功返回站点。

订阅到期或节点变化时，需要更新受限配置、先用 Mihomo `-t -f` 验证，再重启服务并从后端网络测试 OAuth 接口。当前配置使用经过延迟与连通性测试的固定节点，不会自动续订或自动换订阅。不要关闭节点 TLS 验证。

内部网络重建示例（只在不存在时创建；不要改动已经使用的网络）：

```sh
docker network create --internal --subnet 172.30.49.0/29 zhiqiao-oauth-egress
```

订阅网址、节点凭据、控制器密钥及 OAuth Client Secret 均不得提交仓库。备份代理配置时使用受限存储。
