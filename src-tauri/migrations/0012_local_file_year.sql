ALTER TABLE local_files ADD COLUMN tag_year INTEGER NULL;

-- Force one metadata refresh after this migration so existing files pick up
-- their embedded year/date tag on the next local scan.
UPDATE local_files
SET modified_at = -1
WHERE state = 'present';
