# Model Plaza and upstream procurement

The project retains its AGPL-3.0-only license. Model Plaza is available publicly at `/models`, in the user console at `/home/models`, and in the admin console at `/admin/models`. Admin procurement is at `/admin/supply-chain`.

The catalog reads enabled selling prices from ready channels. It supports text, images, video and audio, provider and keyword filters, price sorting, complete channel price options, video tiers, request examples and basic estimates. User sessions receive their group multiplier; public and admin views show base prices. Access and aliases remain governed by the API key's project, so the catalog is not an access list for a particular key.

For integration, configure provider, protocol, endpoint and key/credential in channel management, discover and enable models, diagnose the relevant capability and configure selling prices. Text uses supported OpenAI / Anthropic interfaces; images require a compatible image API; video requires compatible async task creation, polling and retrieval. Protocol compatibility alone does not guarantee every capability.

Procurement quotes are separate admin data. Record supplier, input/output/cache costs or unit costs, monthly budget and notes for each channel/model. Convert supplier currencies into the site settlement currency first. Blank means unknown; zero means free. Amounts are integer micros: one currency unit equals 1,000,000 micros.

The comparison planner evaluates current quotes against hypothetical monthly usage. It does not reconstruct historical procurement invoices. Text scenarios exclude cache costs. Margins use base selling prices and exclude group discounts, taxes and currency movements. Video can record a flat quote per second or per million official tokens, but supplier video tiers are not supported in comparisons. Budgets warn in the planner and do not enforce live spending limits or disable channels. Existing routing policies still determine channel selection.

| Method | Endpoint | Access |
| --- | --- | --- |
| GET | `/api/public/models` | Public base prices |
| GET | `/api/user/models` | User session, group prices |
| GET | `/api/admin/procurement` | Admin |
| POST | `/api/admin/procurement` | Admin, replace a channel/model quote |

Procurement fields: `channel_id`, `model`, `input_cost_micros`, `output_cost_micros`, `cache_read_cost_micros`, `cache_write_cost_micros`, `unit_cost_micros`, `monthly_budget_micros`, `supplier`, `notes`. Send `null` to clear optional amounts. Public catalog responses exclude channel identifiers/names, upstream URLs, keys and procurement data.

Migration `0008_channel_procurement.sql` runs automatically on backend startup. Back up the database and update both frontend and backend through the normal upgrade process. Procurement records cascade when the associated selling price is deleted. Customer billing, balances and selling prices continue independently.

Validation: `pnpm lint`, `pnpm build`, `pnpm test` in `frontend`, and `cargo test --workspace` at the repository root. The optional PostgreSQL publication test runs migrations in an isolated SQLx database:

```sh
DATABASE_URL=postgres://YOUR_USER@localhost/postgres cargo test -p zhiqiaoapi catalog_publication -- --ignored
```

See the [Chinese guide](model-plaza-supply-chain.zh.md) for the detailed workflow and accounting boundaries.
