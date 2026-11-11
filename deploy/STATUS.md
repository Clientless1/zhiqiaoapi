# 京东云迁移状态

业务数据迁移已完成，2026-11-07（北京时间）。公网 IP HTTPS 已恢复并通过外网验证，当前可用地址为 https://111.228.31.204，API Base URL 为 https://111.228.31.204/v1。域名仍受京东云接入限制，用户选择暂不办理备案，详情见 DIRECT-IP-STATUS.md。

- 原服务器：zhiqiao@192.168.1.161
- 京东云服务器：root@111.228.31.204
- 本电脑维护登录：`ssh zhiqiao-jdcloud`。已部署专用密钥并关闭 SSH 密码认证；主机及云端仅放行 22/80/443，Fail2ban、登录限流和浏览器防护已启用。详见 deploy/SECURITY-HARDENING.md。
- 项目：/home/zhiqiao/zhiqiaoapi
- 运行命令：docker compose -f docker-compose.cloud.yml
- 配置域名：https://zhiqiaoapi.fun、https://api.zhiqiaoapi.fun；当前外网域名流量被京东云接入层拦截。
- 网络：按用户要求，域名 DNS 已改为公网 IP 111.228.31.204；Nginx 监听 80/443，回源 127.0.0.1:8090。Cloudflared 已停止并取消自启。
- 完整最终快照：云端 /root/zhiqiao-migration/final
- 自动备份：云端 /home/zhiqiao/backups/zhiqiaoapi/latest
- 已校验：冻结写入后 55 张表 SHA256、46 个序列恢复完全一致，配置及加密密钥匹配。
- 原隧道迁移阶段两个 API Key 公网认证和 GPT 流式请求通过；直连配置后云端正式域名读取模型列表也通过。当前外网域名访问未通过，不能将历史测试视为现状。
- 独立无网络临时 PostgreSQL 完整恢复备份通过。
- 全部业务容器强制重建后健康，账户、Key、余额、账单、用量及设置无变化。模型表仅同步时间及探测统计更新，模型记录无丢失。
- Docker、每小时及开机后业务备份定时器、每日入口备份和 IP 证书续期定时器均已启用。Cloudflared inactive/disabled；IP 续期任务 Result=success。
- 原机三个业务容器均停止，原机 cloudflared 与备份定时器停止并取消自启，原始卷和快照保留。
- 维护说明：JDCLOUD-MIGRATION.md，云端 deploy/JDCLOUD-MIGRATION.md。
- 上线后回退必须先迁回云端新增数据，不能直接恢复旧机对外。

最新云端完整备份已复制到本电脑受限目录，完成位置见 cloud-backup-location.txt。

当前 IP HTTPS 已通过浏览器加载、两个既有 API Key 模型列表和完整 Responses 流式请求验证；停用 Tunnel 后外网健康检查仍为 200，证书校验通过。
