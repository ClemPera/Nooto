CREATE TABLE IF NOT EXISTS `user` (
    `id`                        INT UNSIGNED    NOT NULL AUTO_INCREMENT,
    `username`                  VARCHAR(255)    NOT NULL,
    `stored_password_hash`      TEXT            NOT NULL,
    `stored_recovery_hash`      TEXT            NOT NULL,
    `encrypted_mek_password`    BLOB            NOT NULL,
    `mek_password_nonce`        BLOB            NOT NULL,
    `encrypted_mek_recovery`    BLOB            NOT NULL,
    `mek_recovery_nonce`        BLOB            NOT NULL,
    `salt_auth`                 VARCHAR(255)    NOT NULL,
    `salt_data`                 VARCHAR(255)    NOT NULL,
    `salt_recovery_auth`        VARCHAR(255)    NOT NULL,
    `salt_recovery_data`        VARCHAR(255)    NOT NULL,
    `salt_server_auth`          VARCHAR(255)    NOT NULL,
    `salt_server_recovery`      VARCHAR(255)    NOT NULL,
    PRIMARY KEY (`id`),
    UNIQUE KEY `uq_username` (`username`)
);

CREATE TABLE IF NOT EXISTS `user_token` (
    `id`        INT UNSIGNED    NOT NULL AUTO_INCREMENT,
    `id_user`   INT UNSIGNED    NOT NULL,
    `token`     BLOB            NOT NULL,
    PRIMARY KEY (`id`),
    FOREIGN KEY (`id_user`) REFERENCES `user` (`id`) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS `note` (
    `uuid`              VARCHAR(36)     NOT NULL,
    `id_user`           INT UNSIGNED    NOT NULL,
    `content`           MEDIUMBLOB      NOT NULL,
    `nonce`             BLOB            NOT NULL,
    `metadata`          BLOB            NOT NULL,
    `metadata_nonce`    BLOB            NOT NULL,
    `updated_at`        BIGINT          NOT NULL,
    `deleted`           TINYINT(1)      NOT NULL DEFAULT 0,
    PRIMARY KEY (`uuid`, `id_user`),
    FOREIGN KEY (`id_user`) REFERENCES `user` (`id`) ON DELETE CASCADE
);
