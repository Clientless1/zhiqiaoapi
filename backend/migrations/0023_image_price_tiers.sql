-- Additive and re-entrant: a binary rollback may retain the column and tier data.
ALTER TABLE channel_price ADD COLUMN IF NOT EXISTS image_price_tiers JSONB NOT NULL DEFAULT '[]'::jsonb;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid = 'channel_price'::regclass AND conname = 'channel_price_image_tiers_array') THEN
        ALTER TABLE channel_price ADD CONSTRAINT channel_price_image_tiers_array
            CHECK (jsonb_typeof(image_price_tiers) = 'array');
    END IF;
END $$;
