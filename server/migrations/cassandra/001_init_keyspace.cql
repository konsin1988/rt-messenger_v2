-- ============================================================
-- INIT KEYSPACE
-- ============================================================
CREATE KEYSPACE IF NOT EXISTS messenger
WITH replication = { 'class': 'SimpleStrategy', 'replication_factor': 1 };