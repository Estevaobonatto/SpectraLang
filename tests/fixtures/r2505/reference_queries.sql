INSERT INTO spectra_r2505_users(id, name, score, payload) VALUES ($1, $2, $3, $4);
SELECT id, name, score, payload FROM spectra_r2505_users ORDER BY id;
