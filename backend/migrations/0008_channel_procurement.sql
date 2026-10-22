-- Procurement quotes are private operating data, separate from customer prices.
CREATE TABLE channel_procurement (
    channel_id BIGINT NOT NULL,
    model TEXT NOT NULL,
    input_cost_micros BIGINT CHECK (input_cost_micros BETWEEN 0 AND 1000000000000),
    output_cost_micros BIGINT CHECK (output_cost_micros BETWEEN 0 AND 1000000000000),
    cache_read_cost_micros BIGINT CHECK (cache_read_cost_micros BETWEEN 0 AND 1000000000000),
    cache_write_cost_micros BIGINT CHECK (cache_write_cost_micros BETWEEN 0 AND 1000000000000),
    unit_cost_micros BIGINT CHECK (unit_cost_micros BETWEEN 0 AND 1000000000000),
    monthly_budget_micros BIGINT CHECK (monthly_budget_micros BETWEEN 0 AND 1000000000000),
    supplier TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (channel_id, model),
    FOREIGN KEY (channel_id, model) REFERENCES channel_price(channel_id, model) ON DELETE CASCADE
);

