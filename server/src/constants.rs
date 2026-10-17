/// A token is rejected once it's been idle (unused) for longer than this.
pub const SESSION_TOKEN_MAX_AGE_SECS: i64 = 60 * 60 * 24 * 30;

/// Minimum gap between DB writes that refresh a token's age.
pub const SESSION_TOKEN_REFRESH_AFTER_SECS: i64 = 60 * 60 * 24;


/// Password hash format; see schema::User::password_hash_version.
pub const CURRENT_PASSWORD_HASH_VERSION: u8 = 2;