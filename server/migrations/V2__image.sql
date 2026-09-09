CREATE TABLE IF NOT EXISTS `image` (
    `uuid`          VARCHAR(36)     NOT NULL,
    `id_user`       INT UNSIGNED    NOT NULL,
    `id_note`       VARCHAR(36)     NOT NULL,
    `content`       MEDIUMBLOB      NOT NULL,
    `nonce`         BLOB            NOT NULL,
    `created_at`    BIGINT          NOT NULL,
    PRIMARY KEY (`uuid`),
    FOREIGN KEY (`id_note`, `id_user`) REFERENCES `note` (`uuid`, `id_user`) ON DELETE CASCADE
);
