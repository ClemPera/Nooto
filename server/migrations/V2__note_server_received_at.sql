ALTER TABLE note ADD COLUMN server_received_at BIGINT NOT NULL DEFAULT 0;
UPDATE note SET server_received_at = updated_at WHERE server_received_at = 0
