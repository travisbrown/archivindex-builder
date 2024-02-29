PRAGMA foreign_keys = on;

CREATE TABLE pattern(
    id INTEGER PRIMARY KEY NOT NULL,
    surt TEXT NOT NULL,
    name VARCHAR(255) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    sort_id INTEGER NOT NULL,
    prefix BOOL NOT NULL DEFAULT TRUE,
    active BOOL NOT NULL DEFAULT TRUE,
    updated INTEGER DEFAULT NULL,
    CONSTRAINT uniq_url_surt_prefix UNIQUE (surt, prefix)
);

CREATE INDEX idx_pattern_surt ON pattern (surt);
CREATE UNIQUE INDEX idx_pattern_sort_id ON pattern (sort_id);

CREATE TABLE entry(
    id INTEGER PRIMARY KEY NOT NULL,
    url TEXT NOT NULL,
    surt_id INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    digest VARCHAR(255) NOT NULL,
    mime_type VARCHAR(255) NOT NULL,
    status_code INTEGER,
    length INTEGER NOT NULL,
    FOREIGN KEY (surt_id) REFERENCES surt (id),
    CONSTRAINT uniq_entry_surt_id_ts UNIQUE (surt_id, ts)
);

CREATE INDEX idx_entry_digest ON entry (digest);
CREATE INDEX idx_entry_surt_id ON entry (surt_id);

CREATE TABLE surt(
    id INTEGER PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_surt_value ON surt (value);

CREATE TABLE snapshot(
    id INTEGER PRIMARY KEY NOT NULL,
    digest VARCHAR(255) NOT NULL
);

CREATE UNIQUE INDEX idx_snapshot_digest ON snapshot (digest);

CREATE TABLE entry_success(
    id INTEGER PRIMARY KEY NOT NULL,
    entry_id INTEGER NOT NULL,
    snapshot_id INTEGER NOT NULL,
    correct_digest BOOLEAN,
    ts INTEGER NOT NULL,
    FOREIGN KEY (entry_id) REFERENCES entry (id)
    FOREIGN KEY (snapshot_id) REFERENCES snapshot (id),
    CONSTRAINT uniq_entry_success_entry_id_snapshot_id_correct_digest UNIQUE (entry_id, snapshot_id, correct_digest)
);

CREATE INDEX idx_entry_success_entry_id ON entry_success (entry_id);
CREATE INDEX idx_entry_success_snapshot_id ON entry_success (snapshot_id);

CREATE TABLE entry_failure(
    id INTEGER PRIMARY KEY NOT NULL,
    entry_id INTEGER NOT NULL,
    ts INTEGER NOT NULL,
    status_code INTEGER NOT NULL,
    error_message TEXT NOT NULL,
    FOREIGN KEY (entry_id) REFERENCES entry (id)
);

CREATE INDEX idx_entry_failure_entry_id ON entry_failure (entry_id);

CREATE TABLE link(
    id INTEGER PRIMARY KEY NOT NULL,
    url TEXT NOT NULL,
    surt TEXT NOT NULl
);

CREATE UNIQUE INDEX idx_link_url ON link (url);

CREATE TABLE pattern_entry(
    pattern_id INTEGER NOT NULL,
    entry_id INTEGER NOT NULL,
    CONSTRAINT uniq_pattern_entry_pattern_id_entry_id UNIQUE (pattern_id, entry_id)
);

CREATE INDEX idx_pattern_entry_pattern_id ON pattern_entry (pattern_id);
CREATE INDEX idx_pattern_entry_entry_id ON pattern_entry (entry_id);

CREATE TABLE snapshot_link(
    snapshot_id INTEGER NOT NULL,
    link_id INTEGER NOT NULL,
    CONSTRAINT snapshot_link_snapshot_id_link_id UNIQUE (snapshot_id, link_id)
);

CREATE INDEX idx_snapshot_link_snapshot_id ON snapshot_link (snapshot_id);