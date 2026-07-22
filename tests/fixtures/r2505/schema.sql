CREATE TABLE IF NOT EXISTS spectra_r2505_users (
    id BIGINT PRIMARY KEY,
    name TEXT NOT NULL,
    score DOUBLE PRECISION,
    payload BYTEA
);
