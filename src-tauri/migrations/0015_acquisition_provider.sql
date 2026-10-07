ALTER TABLE app_settings
ADD COLUMN acquisition_provider TEXT NOT NULL DEFAULT 'monochrome'
CHECK (acquisition_provider IN ('monochrome', 'sockseek'));
