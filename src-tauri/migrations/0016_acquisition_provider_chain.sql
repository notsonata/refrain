ALTER TABLE app_settings
ADD COLUMN acquisition_providers_json TEXT NOT NULL DEFAULT '["monochrome"]';

UPDATE app_settings
SET acquisition_providers_json = CASE acquisition_provider
    WHEN 'sockseek' THEN '["sockseek"]'
    ELSE '["monochrome"]'
END;
