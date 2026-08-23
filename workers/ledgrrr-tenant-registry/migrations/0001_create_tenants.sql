CREATE TABLE tenants (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('personal', 'organizational')),
    display_name TEXT NOT NULL,
    root_do_id TEXT NOT NULL,
    created_at TEXT NOT NULL
);
