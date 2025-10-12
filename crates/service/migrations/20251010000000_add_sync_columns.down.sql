-- Rollback sync tracking columns
ALTER TABLE buckets DROP COLUMN sync_error;
ALTER TABLE buckets DROP COLUMN last_sync_attempt;
ALTER TABLE buckets DROP COLUMN sync_status;
