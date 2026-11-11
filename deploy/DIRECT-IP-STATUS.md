# 公网 IP 直连切换记录

更新日期：2026-11-07（北京时间）。

用户已确认使用已有域名 `zhiqiaoapi.fun`，不再使用尚未在当前阿里云账号内注册的 `zhiqiao.fun`。

## 当前可用入口

**公网 IP HTTPS 已恢复：`https://111.228.31.204`，API Base URL 为 `https://111.228.31.204/v1`。用户明确表示当前不办理备案。域名 DNS 绑定保留，但默认域名 HTTPS 仍受京东云接入限制，不能宣布域名可用。**

- 已签发 Let's Encrypt 公网 IP 证书，IP SAN 为 `111.228.31.204`；外网 curl 未使用 `-k` 返回 200、`ssl_verify_result=0`。
- IP 证书使用 `shortlived` profile，有效期约 160 小时；初始证书到期日为 2026-11-13。
- 独立 Certbot 5.9.0 客户端位于 `/opt/zhiqiao-certbot`，证书配置位于 `/etc/letsencrypt-ip`。
- `zhiqiao-ip-cert-renew.timer` 已启用，每日检查两次；续期演练和重载/备份钩子均成功。该任务独立于旧域名证书续期。
- 运行配置已更新为 `PUBLIC_BASE_URL=https://111.228.31.204`，所有业务容器 healthy。
- 浏览器已加载登录页；两个既有 API Key 均通过 IP HTTPS 获取模型列表（200）；GPT Responses 流式请求完成，包含 `response.completed`。
- `cloudflared` 已停止并取消自启；停止后从外网再次访问 IP HTTPS `/readyz` 为 200、正常校验证书。
- 已移除排查时添加的 8443 Nginx 监听和对应京东云防火墙规则，保留原来的 22/80/443 等规则。
- 公网 IP 的 HTTP 入口跳转至相同 IP 的 HTTPS，避免跳转到暂不可用的域名。
- root-only 入口备份现包含 IP 证书、续期任务和原域名配置；业务数据仍按原定时器备份。

## 域名已配置

- 阿里云 DNS：`@`、`api` 的 A 记录均为 `111.228.31.204`，TTL 600 秒。
- 注册局 NS 已切至 `dns27.hichina.com`、`dns28.hichina.com`，直接查询注册局已确认。
- 为旧 NS 缓存同步保留 Cloudflare DNS 的根域名及 api A 记录，均为 DNS only，地址同上。
- 主机 Nginx 监听 80/443，后端仍只通过本机 127.0.0.1:8090 回源。
- 公共证书覆盖 `zhiqiaoapi.fun` 与 `api.zhiqiaoapi.fun`，到期 2027-02-04，私钥仅存服务器及受限备份目录。
- 域名配置阶段曾使用 `PUBLIC_BASE_URL=https://zhiqiaoapi.fun`，当前已改为上述 IP HTTPS；原加密密钥及业务数据保留。
- 数据库、后端、前端容器均 healthy；云端通过正式 HTTPS 域名使用两个既有 Key 读取模型列表均 HTTP 200。
- Certbot 定时器与续期后 Nginx 重载钩子已安装。
- `/root/zhiqiao-origin-backups/latest` 保存主机 Nginx、证书与续期配置的验证备份，按日自动运行。
- 本机 `direct-origin-backup/` 已保存一份 SHA256 校验通过的受限入口配置备份。

## 当前未解决的外网问题

**公网 IP HTTPS 已验证可用；公网域名直连尚未恢复，旧域名证书自动续期仍未通过。IP 证书续期已独立配置并通过演练。**

首次正式证书申请成功，且本机曾直接通过 443 验证 HTTP 200；随后外网域名请求受到应用服务器前方的网络层拦截：

- 本机无 SNI 直接连接 IP 的 HTTPS 请求 HTTP 200。
- 本机带域名的 HTTP 请求及 TLS SNI 握手超时，Nginx 未收到相应请求。
- 三个独立外部节点（美国、俄罗斯、土耳其）访问域名 ACME 检查路径均 HTTP 403；检查编号 `55fbf52bkad6`。
- 续期演练失败，根域名的 ACME 校验返回外部 HTTP 403，api 子域名连接失败；Nginx 无对应请求。
- 本机和云端内部对同一路径均 HTTP 200，Nginx 配置和文件访问权限正常。

已沿用用户原来登录的 Edge，核实京东云轻量云主机控制台：

- 公网 IP `111.228.31.204` 对应北京实例 `lavm-50lwq2sc8k`，运行中，到期日 2027-11-06。
- 实例列表明确提示：“根据工信部及各省（自治区、直辖市）通信管理局要求，部署在京东云的域名或应用须完成备案。未完成备案的接入将面临拦截，影响业务正常访问，请尽快完成备案。”
- 已登录账号的“备案记录”列表显示“暂无数据”。
- “开始备案”进入“验证备案信息”，需要实际所在地区、单位性质、证件类型与号码、服务类型及关联服务等资料。
- 尚需用户确认该域名是否在其他接入商备案，以判断办理首次备案还是接入备案；未填写或提交任何实名资料。
- 本次重新验证：容器全部 healthy；服务器正式域名 `/readyz` 返回 200，本电脑 IP HTTPS 健康检查返回 200（诊断使用 `-k`，IP 与域名证书不匹配），本电脑域名 HTTPS 握手超时。

当前域名阻塞是京东云的备案接入限制。用户明确暂不办理备案，因此先使用上述可信 IP HTTPS 入口；域名恢复另行处理。浏览器控制菜单已按用户授权打开，未重新登录京东云。

控制台证据另存本机 `output/playwright/jdcloud-direct-access-evidence.json`（不包含 Cookie 或令牌）。

Cloudflared 已停止并取消自启，隧道凭据作为回退资料保留。当前 IP HTTPS 不依赖隧道。若未来恢复隧道入口，需要显式恢复 DNS 和服务，不能仅启动进程。

原域名 NS 备查：`fatima.ns.cloudflare.com`、`lynn.ns.cloudflare.com`。原 api 记录是代理 CNAME，目标 `f5277016-7d2f-4229-981e-c32644028c4b.cfargotunnel.com`。

## 维护

2026-11-07 已完成公网入口加固：本电脑用 `ssh zhiqiao-jdcloud` 密钥登录，密码 SSH 已关闭；主机与京东云仅放行22/80/443。已启用 SSH 自动封禁、认证限流、安全响应头和敏感路径拦截。安全配置随每日 root-only 入口归档备份，执行脚本为 `/usr/local/sbin/zhiqiao-origin-backup.py`；完整记录见 deploy/SECURITY-HARDENING.md。

```sh
nginx -t
systemctl status nginx zhiqiao-ip-cert-renew.timer --no-pager
systemctl start zhiqiao-origin-backup.service
systemctl show zhiqiao-origin-backup.service -p Result -p ExecMainStatus
/opt/zhiqiao-certbot/bin/certbot renew --dry-run --no-random-sleep-on-renew --run-deploy-hooks --config-dir /etc/letsencrypt-ip --work-dir /var/lib/letsencrypt-ip --logs-dir /var/log/letsencrypt-ip
```

配置文件 `/etc/nginx/sites-available/zhiqiao-direct`；项目中示例 `deploy/nginx/direct-origin.conf.example`。配置恢复涉及证书私钥，备份不可提交 Git 或公开。

## IP 证书恢复

恢复 `/etc/nginx`、`/etc/letsencrypt-ip` 和相应 systemd 单元后，需恢复 Certbot 运行环境：

```sh
python3 -m venv /opt/zhiqiao-certbot
/opt/zhiqiao-certbot/bin/python -m pip install -r /home/zhiqiao/zhiqiaoapi/deploy/ops/ip-certbot-requirements.txt
nginx -t
systemctl daemon-reload
systemctl enable --now nginx zhiqiao-ip-cert-renew.timer
systemctl start zhiqiao-ip-cert-renew.service
```

IP 证书依赖 80 端口的 `/.well-known/acme-challenge/` 可访问，不能删除该路径或关闭 80 入站规则。证书私钥、账户密钥与运行环境备份均不可公开。
