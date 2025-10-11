-- Add sync tracking columns to buckets table
ALTER TABLE buckets ADD COLUMN sync_status TEXT NOT NULL DEFAULT 'synced';
ALTER TABLE buckets ADD COLUMN last_sync_attempt TIMESTAMP;
ALTER TABLE buckets ADD COLUMN sync_error TEXT;
