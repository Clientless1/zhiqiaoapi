# 京东云迁移与维护

业务迁移完成日期：2026-11-07（北京时间）。

当前可用入口：https://111.228.31.204；API Base URL：https://111.228.31.204/v1。IP 使用可信证书，自动续期和流式 API 已验证。用户暂不备案，域名 DNS 已绑定但仍受京东云域名接入限制。Cloudflared 已停止并取消自启，完整现状见 DIRECT-IP-STATUS.md。

- 新服务器：111.228.31.204
- 本电脑 SSH：`ssh zhiqiao-jdcloud`；专用密钥位于本电脑 `~/.ssh/zhiqiao-jdcloud_ed25519`。SSH 密码登录已关闭，密钥已独立验证。不要删除该密钥或将其提交 GitHub。
- 项目目录：/home/zhiqiao/zhiqiaoapi
- 配置入口：https://zhiqiaoapi.fun、https://api.zhiqiaoapi.fun（当前受云平台备案接入限制）。
- 域名已改为公网 IP 直连 Nginx，再回源到京东云本机 127.0.0.1:8090；公共 DNS 当前不走 Cloudflare Tunnel。数据库和后端端口仅在 Docker 网络内访问。
- 当前运行镜像从原服务器导入，与迁移时线上运行版本一致。
- 生产 Compose：docker-compose.cloud.yml。不要使用不带该文件的命令操作当前服务。
- 运行配置及渠道加密密钥随数据库一同恢复，未重建用户账户或 API Key。

## 查看状态与重启

```sh
cd /home/zhiqiao/zhiqiaoapi
docker compose -f docker-compose.cloud.yml ps
curl -fsS http://127.0.0.1:8090/readyz
systemctl status nginx zhiqiao-ip-cert-renew.timer --no-pager
```

已启用 Docker 和 Nginx 开机启动，Cloudflared 已停止并取消自启，三个业务容器均为 unless-stopped。正常容器重建仍使用固定 external 数据卷。

```sh
docker compose -f docker-compose.cloud.yml up -d --no-build --wait
```

需要从仓库源码重新构建时，先构建并确认 zhiqiaoapi-backend、zhiqiaoapi-web 镜像，再使用生产 Compose 更新；保留旧镜像以便回退。

## 备份

2026-11-07 安全加固后，UFW 与云防火墙均仅放行 TCP22/80/443；Fail2ban 自动封禁 SSH 失败尝试。认证入口每 IP10次/分钟、burst10，超限返回429；正常模型 API 不受该认证额度影响。每日系统安全更新已开启，自动重启关闭。详细配置与检查命令见 deploy/SECURITY-HARDENING.md。

每日入口归档 `/root/zhiqiao-origin-backups/latest` 另含 Nginx、TLS 私钥、SSH、UFW、Fail2ban 与 APT 安全配置，目录0700、文件0600。root 任务实际执行 `/usr/local/sbin/zhiqiao-origin-backup.py`，不执行普通用户项目目录中的同名副本。

每小时及开机后自动运行完整备份。内容包括数据库、角色、配置密钥、资源和部署源码；目录为 /home/zhiqiao/backups/zhiqiaoapi。

```sh
systemctl list-timers zhiqiaoapi-backup.timer --no-pager
systemctl show zhiqiaoapi-backup.service -p Result -p ExecMainStatus
runuser -u zhiqiao -- python3 /home/zhiqiao/zhiqiaoapi/deploy/ops/backup-zhiqiaoapi.py
runuser -u zhiqiao -- python3 /home/zhiqiao/zhiqiaoapi/deploy/ops/verify-restore.py
```

迁移期间的初始备份、冻结写入后的最终快照及校验报告位于 /root/zhiqiao-migration。该目录含敏感数据，不能上传公共仓库。

## 原隧道迁移阶段的校验记录（不能替代当前直连外网验证）

- 最终冻结数据的 55 张表逐表 SHA-256 和 46 个序列，在云端恢复后完全一致。
- 运行配置（包括加密密钥）逐文件摘要一致。
- 两个现有 API Key 经正式域名读取模型列表均返回 HTTP 200。
- 正式域名 Responses 流式请求完成，模型回复 OK。
- 完整备份在无网络、无公开端口的独立临时 PostgreSQL 中恢复成功。
- 容器重建后所有账户、Key、余额、账单、用量、设置及模型记录均保留。模型同步会刷新发现时间、最后探测时间，并使相关自增序列前进；已确认没有丢失任何模型记录。详细校验见 /root/zhiqiao-migration 中 inventory 报告。

## 原服务器与回退

原服务器 192.168.1.161 的数据库卷、资源卷、配置卷和迁移快照保留。原业务容器、原 cloudflared 隧道与原备份定时器均已停止；隧道和定时器已取消开机启动，防止旧实例重新接收流量。

京东云上线后会产生新数据。回退前必须先暂停云端写入，生成新的完整快照，将数据库、配置密钥及资源恢复到原机并核验，再停用云端隧道、启用原机隧道。不得直接启动原机旧数据库对外服务，否则会遗漏云端上线后的数据。
