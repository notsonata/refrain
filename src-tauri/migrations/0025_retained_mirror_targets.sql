-- Forgetting a destination hides it without losing transfer history.
ALTER TABLE mirror_targets ADD COLUMN active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1));
