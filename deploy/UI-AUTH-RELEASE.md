# UI and authentication release — 2026-11-07

This release separates password login, email login, and registration; adds optional GitHub/Google OAuth; adds administrator control of user navigation; and replaces the public homepage, authentication pages, model cards, and notification presentation.

## Configuration

- User menu visibility: administrator console → settings → user navigation. Overview and personal settings remain available. Hiding a page controls the user console navigation and route; it does not change API authorization or the public site's navigation.
- OAuth: administrator console → settings → authentication. Both providers default to disabled. Register the callback displayed by the page with the provider, then enter the Client ID and Client Secret. Stored secrets are encrypted and never returned by the settings API. Google requires an accepted domain callback; the current IP address cannot serve as a Google production callback.
- Password login requires an existing registered user and no email code. Email login requires an existing user and a login-purpose code. Registration requires a registration-purpose code and explicit agreement acceptance. OAuth does not automatically link an existing account by matching email.

## Production deployment

Host: the existing `zhiqiao-jdcloud` SSH alias. Project: `/home/zhiqiao/zhiqiaoapi`. Use **docker-compose.cloud.yml**, whose database/configuration/assets volumes are external. Never run volume removal commands during deployment.

Prepared image tags: `zhiqiaoapi-web:ui-auth-20261107` and `zhiqiaoapi-backend:ui-auth-20261107`. Previous images are retained as `before-ui-auth-20261107` for both services. Deployment source archives and build logs are in `/home/zhiqiao/build-ui-auth`.

Before switching images, run `systemctl start zhiqiaoapi-backup.service` and confirm its successful result and the new `/home/zhiqiao/backups/zhiqiaoapi/latest` backup. Preserve production environment files and cloud compose/nginx configuration when updating source.

Tag the prepared images as `latest` and run `docker compose -f docker-compose.cloud.yml up -d --no-build --pull never backend web` from the project directory. Wait for all health checks and check `/readyz`, `/api/auth/providers`, `/api/public/navigation`, and the new HTML/asset hashes from the public HTTPS address. Migration 0018 is additive and applies on backend startup.

## Rollback

Retag the two `before-ui-auth-20261107` images as `latest`, then run the same compose command. Keep the additive migration in place; routine image rollback must not overwrite the database or lose transactions made after release. Database restoration is only for an actual data recovery incident.

## Validation

Frontend typecheck, ESLint, 74 tests, and production build passed. Backend authentication compilation and 18 authentication tests passed, including temporary PostgreSQL integration checks. Four navigation normalization tests passed. Public homepage, model cards/comparison, and login/registration layouts were checked in the browser at desktop and 390px mobile widths. Actual third-party OAuth login requires the operator's provider credentials and has not been exercised.

## Deployment result

Deployed at approximately 2026-11-07 15:57 UTC. The pre-release backup is `/home/zhiqiao/backups/zhiqiaoapi/20261107T155033117224Z`. All three production containers reported healthy, and the scheduler successfully probed its existing upstream after restart. The public page loaded `/assets/index-CSJAk23_.js`; public readiness/providers/navigation endpoints returned 200; protected settings endpoints rejected unauthenticated requests with 401. An unregistered username/password login returned 401 without requesting an email code.

The native Linux build required GLIBC 2.38, while the retained runtime image uses GLIBC 2.36. The deployed overlay keeps the newer loader and required libraries isolated under `/opt/zhiqiao-runtime`, and only the new backend binary uses them through a wrapper. Existing runtime libraries and the scheduler remain intact. Its Dockerfile, wrapper, and inputs are retained at `/home/zhiqiao/build-ui-auth/image-compatible`. This image was tested against a disposable isolated PostgreSQL container before production deployment: readiness passed, migration 18 completed, and the new public endpoints worked. The temporary containers/network were removed.

An existing production user session remained valid after deployment. The user console, live model cards, API key page, and both notification entry points were checked in Edge with no console errors. That account has no notifications, so the live notification check exercised the empty state. Screenshots are saved in the task workspace's `artifacts/ui-auth-20261107` directory.
