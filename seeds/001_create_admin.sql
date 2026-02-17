INSERT INTO users (username, email, password_hash, is_admin)
VALUES (
    'admin',
    'admin@genso.kyo',
    '$2a$12$ai3ZiL0LL3aZFB1pVXnqneCKB1Lrsv.jwRuF55XN5LXfI.ENrI8u2',
    true
)
ON CONFLICT (email) DO NOTHING;