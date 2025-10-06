CREATE TABLE buckets (
    -- the global identifier for the bucket
    id TEXT PRIMARY KEY,
    -- the friendly name of the bucket
    name TEXT NOT NULL,
    -- the link to the current version of the bucket, as a bas58 cid
    link VARCHAR(255) NOT NULL,

    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX buckets_id_name ON buckets (id, name);
