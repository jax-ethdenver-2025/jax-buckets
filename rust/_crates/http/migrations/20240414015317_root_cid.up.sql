/* Root Cid pointer */

CREATE TABLE root_cids (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    cid VARCHAR(255) NOT NULL,
    previous_cid VARCHAR(255) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX root_cids_cid_previous_cid ON root_cids (cid, previous_cid);
