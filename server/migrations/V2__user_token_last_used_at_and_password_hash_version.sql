ALTER TABLE `user_token` ADD COLUMN IF NOT EXISTS `last_used_at` BIGINT NOT NULL DEFAULT 0;
UPDATE `user_token` SET `last_used_at` = UNIX_TIMESTAMP() WHERE `last_used_at` = 0;
ALTER TABLE `user` ADD COLUMN IF NOT EXISTS `password_hash_version` TINYINT UNSIGNED NOT NULL DEFAULT 1;
