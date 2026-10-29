CREATE TABLE IF NOT EXISTS `server_secret` (
    `name`  VARCHAR(64) NOT NULL,
    `value` BLOB        NOT NULL,
    PRIMARY KEY (`name`)
);
